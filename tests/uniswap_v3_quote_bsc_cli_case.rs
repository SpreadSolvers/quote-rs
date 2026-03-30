//! BSC integration: same inputs as failing CLI run (PancakeSwap V3 pool, USDT → TOKEN_OUT).
//!
//! ```text
//! quote 0x8f889728C2a879B15936eecC38A61F03fCDc6818 uni-v3 \
//!   0x55d398326f99059fF775485246999027B3197955 500000000000000000000 <BSC_RPC>
//! ```
//!
//! Set `BSC_RPC_URL` to run (see `#[ignore]` on the test).
//!
//! Regression: PancakeSwap V3 pools call `pancakeV3SwapCallback`; the quoter must implement it (see Solidity).

use alloy::primitives::{Address, U256};
use alloy::sol;
use quote::provider::create_provider;
use quote::uniswap_v3;
use std::str::FromStr;

sol! {
    #[sol(rpc)]
    interface IQuoterV2 {
        struct QuoteExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint256 amountIn;
            uint24 fee;
            uint160 sqrtPriceLimitX96;
        }

        function quoteExactInputSingle(QuoteExactInputSingleParams params)
            external
            returns (uint256 amountOut, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate);
    }
}

const POOL: &str = "0x8f889728C2a879B15936eecC38A61F03fCDc6818";
const USDT: &str = "0x55d398326f99059fF775485246999027B3197955";
const TOKEN_OUT: &str = "0xb150e91Cb40909F47d45115eE9E90667D807464B";
const QUOTER_V2_BSC: &str = "0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997";

/// BSC-only: do not fall back to `RPC_URL` (often an Ethereum endpoint).
fn bsc_rpc_url() -> Option<String> {
    std::env::var("BSC_RPC_URL").ok().filter(|u| !u.is_empty())
}

async fn quoter_v2_amount_out(
    provider: &quote::provider::MyProvider,
    token_in: Address,
    token_out: Address,
    amount_in: u128,
) -> u128 {
    let quoter = IQuoterV2::new(Address::from_str(QUOTER_V2_BSC).unwrap(), provider);
    let params = IQuoterV2::QuoteExactInputSingleParams {
        tokenIn: token_in,
        tokenOut: token_out,
        amountIn: U256::from(amount_in),
        fee: alloy::primitives::Uint::<24, 1>::from(100),
        sqrtPriceLimitX96: alloy::primitives::U160::ZERO,
    };
    let ret = quoter
        .quoteExactInputSingle(params)
        .call()
        .await
        .expect("Pancake QuoterV2 quoteExactInputSingle");
    ret.amountOut.to::<u128>()
}

#[tokio::test]
#[ignore = "Set BSC_RPC_URL and run: cargo test --test uniswap_v3_quote_bsc_cli_case -- --ignored --nocapture"]
async fn uniswap_v3_bsc_cli_case_matches_quoter_v2() {
    dotenv::dotenv().ok();
    let rpc_url = bsc_rpc_url().expect("BSC_RPC_URL required for this integration test");
    let provider = create_provider(&rpc_url).await.expect("provider");

    let pool = Address::from_str(POOL).unwrap();
    let token_in = Address::from_str(USDT).unwrap();
    let token_out = Address::from_str(TOKEN_OUT).unwrap();
    let amount_in = 500u128 * 10u128.pow(18);

    let quoted = uniswap_v3::quote(pool, token_in, amount_in, 0, provider.clone())
        .await
        .expect("uniswap_v3::quote (ephemeral deploy)");

    let baseline = quoter_v2_amount_out(&provider, token_in, token_out, amount_in).await;

    assert_eq!(
        quoted, baseline,
        "ephemeral quote ({quoted}) != Pancake QuoterV2 ({baseline}) — if RPC returned empty revert, try another BSC endpoint"
    );
}
