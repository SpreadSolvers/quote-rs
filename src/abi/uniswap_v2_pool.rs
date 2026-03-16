use alloy::sol;

sol! {
    #[sol(rpc)]
    interface UniswapV2Pool {
        function factory() view returns (address);
    }
}
