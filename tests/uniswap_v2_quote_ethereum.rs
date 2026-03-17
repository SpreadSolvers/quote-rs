//! Integration test: Uniswap V2 quote matches Router getAmountsOut on Ethereum mainnet.
//!
//! Pool: USDC/WETH 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc.
//! Set ETHEREUM_RPC_URL (or RPC_URL) to run. For reproducible results use a fork URL
//! with a fixed block (e.g. anvil --fork-url <rpc> --fork-block-number 21500000).

use alloy::primitives::Address;
use alloy::sol;
use quote::provider::create_provider;
use quote::{DEFAULT_UNISWAP_V2_FEE_BPS, uniswap_v2};
use std::str::FromStr;

sol! {
    #[sol(rpc)]
    interface IUniswapV2Router02 {
        function getAmountsOut(uint256 amountIn, address[] path) external view returns (uint256[] amounts);
    }
}

const USDC_ETH_POOL: &str = "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc";
const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
const WETH: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
const ROUTER: &str = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D";
/// 1 USDC (6 decimals)
const AMOUNT_IN: u128 = 1_000_000u128;

#[tokio::test]
async fn uniswap_v2_quote_matches_router_get_amounts_out_usdc_eth() {
    let rpc_url = std::env::var("ETHEREUM_RPC_URL").or_else(|_| std::env::var("RPC_URL"));
    let rpc_url = match rpc_url {
        Ok(u) if !u.is_empty() => u,
        _ => {
            eprintln!("Skip: set ETHEREUM_RPC_URL or RPC_URL to run");
            return;
        }
    };

    let pool_id = Address::from_str(USDC_ETH_POOL).unwrap();
    let token_in = Address::from_str(USDC).unwrap();
    let router_addr = Address::from_str(ROUTER).unwrap();

    let provider = create_provider(&rpc_url).await.expect("provider");
    let router = IUniswapV2Router02::new(router_addr, &provider);

    let path = vec![token_in, Address::from_str(WETH).unwrap()];

    let quoted = uniswap_v2::quote(
        pool_id,
        token_in,
        AMOUNT_IN,
        DEFAULT_UNISWAP_V2_FEE_BPS,
        provider.clone(),
    )
    .await
    .expect("quote");

    let amounts = router
        .getAmountsOut(alloy::primitives::U256::from(AMOUNT_IN), path)
        .call()
        .await
        .expect("getAmountsOut");

    let router_amount_out = amounts.last().copied().unwrap_or_default();
    let router_amount_out_u128: u128 = router_amount_out.to::<u128>();

    assert_eq!(
        quoted, router_amount_out_u128,
        "quote ({quoted}) != Router getAmountsOut ({router_amount_out_u128})"
    );
}
