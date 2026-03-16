use alloy::sol;

sol! {
    interface IUniswapV2Pair {
        event Approval(address indexed owner, address indexed spender, uint256 value);
        event Transfer(address indexed from, address indexed to, uint256 value);

        function name() external pure returns (string memory);
        function symbol() external pure returns (string memory);
        function decimals() external pure returns (uint8);
        function totalSupply() external view returns (uint256);
        function balanceOf(address owner) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);

        function approve(address spender, uint256 value) external returns (bool);
        function transfer(address to, uint256 value) external returns (bool);
        function transferFrom(address from, address to, uint256 value) external returns (bool);

        function DOMAIN_SEPARATOR() external view returns (bytes32);
        function PERMIT_TYPEHASH() external pure returns (bytes32);
        function nonces(address owner) external view returns (uint256);

        function permit(address owner, address spender, uint256 value, uint256 deadline, uint8 v, bytes32 r, bytes32 s)
            external;

        event Mint(address indexed sender, uint256 amount0, uint256 amount1);
        event Burn(address indexed sender, uint256 amount0, uint256 amount1, address indexed to);
        event Swap(
            address indexed sender,
            uint256 amount0In,
            uint256 amount1In,
            uint256 amount0Out,
            uint256 amount1Out,
            address indexed to
        );
        event Sync(uint112 reserve0, uint112 reserve1);

        function MINIMUM_LIQUIDITY() external pure returns (uint256);
        function factory() external view returns (address);
        function token0() external view returns (address);
        function token1() external view returns (address);
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
        function price0CumulativeLast() external view returns (uint256);
        function price1CumulativeLast() external view returns (uint256);
        function kLast() external view returns (uint256);

        function mint(address to) external returns (uint256 liquidity);
        function burn(address to) external returns (uint256 amount0, uint256 amount1);
        function swap(uint256 amount0Out, uint256 amount1Out, address to, bytes calldata data) external;
        function skim(address to) external;
        function sync() external;

        function initialize(address, address) external;
    }


    interface IUniswapV2Factory {
        function getPair(address tokenA, address tokenB) external view returns (address pair);
    }

    /**
     * @notice Quotes output for Uniswap V2 / Voltage-style pairs.
     * @dev Pair uses 0.3% fee (balance*1000 - amountIn*3). If the protocol's router/frontend
     *      applies an extra protocol fee (e.g. Voltage), set protocolFeeBps so the quote matches.
     */
    #[sol(rpc, bytecode="608080604052346102905760808161037b803803809161001f8285610306565b833981010312610290576100328161033f565b9061003f6020820161033f565b6040808301516060938401519151630240bc6b60e21b815291946001600160a01b03169390929082600481875afa91821561029d5760009081936102a9575b50604051630dfe168160e01b81526001600160701b03938416959390911692602090829060049082905afa90811561029d5760009161025e575b506001600160a01b039182169116036102585791905b80156101ff578215918215806101f6575b156101a0576103e582029182046103e503610169576100fe9082610367565b916103e884029384046103e814171561016957820180921161016957811561018a570480918015158061017f575b610146575b8263bbd6b10b60e01b60005260045260246000fd5b61271090810392508211610169576127109161016191610367565b043880610131565b634e487b7160e01b600052601160045260246000fd5b50612710811061012c565b634e487b7160e01b600052601260045260246000fd5b60405162461bcd60e51b815260206004820152602860248201527f556e697377617056324c6962726172793a20494e53554646494349454e545f4c604482015267495155494449545960c01b6064820152608490fd5b508015156100df565b60405162461bcd60e51b815260206004820152602b60248201527f556e697377617056324c6962726172793a20494e53554646494349454e545f4960448201526a1394155517d05353d5539560aa1b6064820152608490fd5b906100ce565b90506020813d602011610295575b8161027960209383610306565b810103126102905761028a9061033f565b386100b8565b600080fd5b3d915061026c565b6040513d6000823e3d90fd5b92506060833d6060116102fe575b816102c460609383610306565b810103126102fb576102d583610353565b9060406102e460208601610353565b94015163ffffffff8116036102fb5750602061007e565b80fd5b3d91506102b7565b601f909101601f19168101906001600160401b0382119082101761032957604052565b634e487b7160e01b600052604160045260246000fd5b51906001600160a01b038216820361029057565b51906001600160701b038216820361029057565b818102929181159184041417156101695756fe")]
    contract UniswapV2QuoteSingle {
        error AmountOut(uint256 amountOut);

        /**
         * @param pool         Pair address.
         * @param amountIn         Input amount (path[0] token).
         * @param tokenIn          Input token address.
         * @param protocolFeeBps   Optional fee in basis points (e.g. 300 = 3%) deducted from amountOut
         *                         to match frontend "amount received" when router takes a cut. Use 0 for raw pair quote.
         */
        constructor(address pool, address tokenIn, uint256 amountIn, uint256 protocolFeeBps) {
            (uint256 reserve0, uint256 reserve1,) = IUniswapV2Pair(pool).getReserves();

            address token0 = IUniswapV2Pair(pool).token0();

            (uint256 reserveIn, uint256 reserveOut) = tokenIn == token0 ? (reserve0, reserve1) : (reserve1, reserve0);

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
            pair = address(
                uint160(
                    uint256(
                        keccak256(
                            abi.encodePacked(
                                "ff",
                                factory,
                                keccak256(abi.encodePacked(token0, token1)),
                                "96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f" // init code hash
                            )
                        )
                    )
                )
            );
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
