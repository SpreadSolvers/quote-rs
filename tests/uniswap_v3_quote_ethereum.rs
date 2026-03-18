//! Integration test: Uniswap V3 quote matches QuoterV2 on Ethereum mainnet.
//!
//! Pool: USDC/WETH 0.3% `0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8`.
//! Set ETHEREUM_RPC_URL or RPC_URL to run.

use alloy::contract::Error as ContractError;
use alloy::primitives::{Address, U256};
use alloy::sol;
use alloy::sol_types::SolValue;
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

const POOL: &str = "0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8";
const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
const WETH: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
const QUOTER_V2: &str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";

fn rpc_url() -> Option<String> {
    std::env::var("ETHEREUM_RPC_URL")
        .or_else(|_| std::env::var("RPC_URL"))
        .ok()
        .filter(|u| !u.is_empty())
}

fn revert_data_from_error(e: &ContractError) -> Option<alloy::primitives::Bytes> {
    if let Some(data) = e.as_revert_data() {
        return Some(data);
    }
    let ContractError::TransportError(te) = e else {
        return None;
    };
    let payload = te.as_error_resp()?;
    let raw = payload.data.as_ref()?;
    let s = raw.get().trim_matches('"').trim();
    let hex_str = s
        .strip_prefix("Reverted 0x")
        .or_else(|| s.strip_prefix("0x"))?;
    hex::decode(hex_str)
        .ok()
        .map(alloy::primitives::Bytes::from)
}

async fn quoter_v2_amount_out(
    provider: &quote::provider::MyProvider,
    token_in: Address,
    token_out: Address,
    amount_in: u128,
) -> u128 {
    let quoter = IQuoterV2::new(Address::from_str(QUOTER_V2).unwrap(), provider);
    let params = IQuoterV2::QuoteExactInputSingleParams {
        tokenIn: token_in,
        tokenOut: token_out,
        amountIn: U256::from(amount_in),
        fee: alloy::primitives::Uint::<24, 1>::from(3000),
        sqrtPriceLimitX96: alloy::primitives::U160::ZERO,
    };

    match quoter.quoteExactInputSingle(params).call().await {
        Ok(ret) => ret.amountOut.to::<u128>(),
        Err(e) => {
            let data = revert_data_from_error(&e)
                .unwrap_or_else(|| panic!("QuoterV2 revert data not available: {e:?}"));
            assert_eq!(data.len(), 96, "QuoterV2 revert should be 96 bytes");
            let (amount_out, _, _): (U256, alloy::primitives::U160, i32) =
                SolValue::abi_decode(data.as_ref()).expect("decode QuoterV2 revert");
            amount_out.to::<u128>()
        }
    }
}

#[tokio::test]
async fn uniswap_v3_quote_matches_quoter_v2_usdc_to_weth() {
    dotenv::dotenv().ok();
    let rpc_url = rpc_url().expect("ETHEREUM_RPC_URL or RPC_URL required for integration test");
    let provider = create_provider(&rpc_url).await.expect("provider");

    let pool_id = Address::from_str(POOL).unwrap();
    let token_in = Address::from_str(USDC).unwrap();
    let token_out = Address::from_str(WETH).unwrap();
    let amount_in = 1_000_000u128; // 1 USDC

    let quoted = uniswap_v3::quote(pool_id, token_in, amount_in, 0, provider.clone())
        .await
        .expect("quote");

    let baseline = quoter_v2_amount_out(&provider, token_in, token_out, amount_in).await;

    assert_eq!(
        quoted, baseline,
        "uniswap_v3::quote ({quoted}) != QuoterV2 ({baseline})"
    );
}

#[tokio::test]
async fn uniswap_v3_quote_matches_quoter_v2_weth_to_usdc() {
    dotenv::dotenv().ok();
    let rpc_url = rpc_url().expect("ETHEREUM_RPC_URL or RPC_URL required for integration test");
    let provider = create_provider(&rpc_url).await.expect("provider");

    let pool_id = Address::from_str(POOL).unwrap();
    let token_in = Address::from_str(WETH).unwrap();
    let token_out = Address::from_str(USDC).unwrap();
    let amount_in = 10u128.pow(18); // 1 WETH

    let quoted = uniswap_v3::quote(pool_id, token_in, amount_in, 0, provider.clone())
        .await
        .expect("quote");

    let baseline = quoter_v2_amount_out(&provider, token_in, token_out, amount_in).await;

    assert_eq!(
        quoted, baseline,
        "uniswap_v3::quote ({quoted}) != QuoterV2 ({baseline})"
    );
}
