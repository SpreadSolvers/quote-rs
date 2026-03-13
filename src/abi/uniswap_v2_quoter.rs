use alloy::sol;

sol! {
    interface IUniswapV2Factory {
        function getPair(address tokenA, address tokenB) external view returns (address pair);
    }

    /**
     * @notice Quotes output for Uniswap V2 / Voltage-style pairs.
     * @dev Pair uses 0.3% fee (balance*1000 - amountIn*3). If the protocol's router/frontend
     *      applies an extra protocol fee (e.g. Voltage), set protocolFeeBps so the quote matches.
     */
    #[sol(rpc)]
    contract UniswapV2Quoter {
        error AmountOut(uint256 amountOut);

        /**
         * @param _factory         Pair factory (e.g. Voltage).
         * @param amountIn         Input amount (path[0] token).
         * @param path             [tokenIn, tokenOut].
         * @param protocolFeeBps   Optional fee in basis points (e.g. 300 = 3%) deducted from amountOut
         *                         to match frontend "amount received" when router takes a cut. Use 0 for raw pair quote.
         */
        constructor(address _factory, uint256 amountIn, address[] memory path, uint256 protocolFeeBps) {
            address pair = IUniswapV2Factory(_factory).getPair(path[0], path[1]);
            (uint256 reserve0, uint256 reserve1,) = IUniswapV2Pair(pair).getReserves();

            (uint256 reserveIn, uint256 reserveOut) = path[0] < path[1] ? (reserve0, reserve1) : (reserve1, reserve0);

            uint256 amountOut = UniswapV2Library.getAmountOut(amountIn, reserveIn, reserveOut);
            if (protocolFeeBps > 0 && protocolFeeBps < 10_000) {
                amountOut = (amountOut * (10_000 - protocolFeeBps)) / 10_000;
            }
            revert AmountOut(amountOut);
        }
    }

    library UniswapV2Library {
        // returns sorted token addresses, used to handle return values from pairs sorted in this order
        function sortTokens(address tokenA, address tokenB) internal pure returns (address token0, address token1) {
            require(tokenA != tokenB, "UniswapV2Library: IDENTICAL_ADDRESSES");
            (token0, token1) = tokenA < tokenB ? (tokenA, tokenB) : (tokenB, tokenA);
            require(token0 != address(0), "UniswapV2Library: ZERO_ADDRESS");
        }

        // calculates the CREATE2 address for a pair without making any external calls
        function pairFor(address factory, address tokenA, address tokenB) internal pure returns (address pair) {
            (address token0, address token1) = sortTokens(tokenA, tokenB);
            pair = address(0);
        }

        // fetches and sorts the reserves for a pair
        function getReserves(address factory, address tokenA, address tokenB)
            internal
            view
            returns (uint256 reserveA, uint256 reserveB)
        {
            (address token0,) = sortTokens(tokenA, tokenB);
            (uint256 reserve0, uint256 reserve1,) = IUniswapV2Pair(pairFor(factory, tokenA, tokenB)).getReserves();
            (reserveA, reserveB) = tokenA == token0 ? (reserve0, reserve1) : (reserve1, reserve0);
        }

        // given some amount of an asset and pair reserves, returns an equivalent amount of the other asset
        function quote(uint256 amountA, uint256 reserveA, uint256 reserveB) internal pure returns (uint256 amountB) {
            require(amountA > 0, "UniswapV2Library: INSUFFICIENT_AMOUNT");
            require(reserveA > 0 && reserveB > 0, "UniswapV2Library: INSUFFICIENT_LIQUIDITY");
            amountB = (amountA * reserveB) / reserveA;
        }

        // given an input amount of an asset and pair reserves, returns the maximum output amount of the other asset
        function getAmountOut(uint256 amountIn, uint256 reserveIn, uint256 reserveOut)
            internal
            pure
            returns (uint256 amountOut)
        {
            require(amountIn > 0, "UniswapV2Library: INSUFFICIENT_INPUT_AMOUNT");
            require(reserveIn > 0 && reserveOut > 0, "UniswapV2Library: INSUFFICIENT_LIQUIDITY");
            uint256 amountInWithFee = amountIn * 997;
            uint256 numerator = amountInWithFee * reserveOut;
            uint256 denominator = reserveIn * 1000 + amountInWithFee;
            amountOut = numerator / denominator;
        }

        // given an output amount of an asset and pair reserves, returns a required input amount of the other asset
        function getAmountIn(uint256 amountOut, uint256 reserveIn, uint256 reserveOut)
            internal
            pure
            returns (uint256 amountIn)
        {
            require(amountOut > 0, "UniswapV2Library: INSUFFICIENT_OUTPUT_AMOUNT");
            require(reserveIn > 0 && reserveOut > 0, "UniswapV2Library: INSUFFICIENT_LIQUIDITY");
            uint256 numerator = reserveIn * amountOut * 1000;
            uint256 denominator = (reserveOut - amountOut) * 997;
            amountIn = (numerator / denominator) + 1;
        }

        // performs chained getAmountOut calculations on any number of pairs
        function getAmountsOut(address factory, uint256 amountIn, address[] memory path)
            internal
            view
            returns (uint256[] memory amounts)
        {
            require(path.length >= 2, "UniswapV2Library: INVALID_PATH");
            amounts = new uint256[](path.length);
            amounts[0] = amountIn;
            for (uint256 i; i < path.length - 1; i++) {
                (uint256 reserveIn, uint256 reserveOut) = getReserves(factory, path[i], path[i + 1]);
                amounts[i + 1] = getAmountOut(amounts[i], reserveIn, reserveOut);
            }
        }

        // performs chained getAmountIn calculations on any number of pairs
        function getAmountsIn(address factory, uint256 amountOut, address[] memory path)
            internal
            view
            returns (uint256[] memory amounts)
        {
            require(path.length >= 2, "UniswapV2Library: INVALID_PATH");
            amounts = new uint256[](path.length);
            amounts[amounts.length - 1] = amountOut;
            for (uint256 i = path.length - 1; i > 0; i--) {
                (uint256 reserveIn, uint256 reserveOut) = getReserves(factory, path[i - 1], path[i]);
                amounts[i - 1] = getAmountIn(amounts[i], reserveIn, reserveOut);
            }
        }
    }
}
