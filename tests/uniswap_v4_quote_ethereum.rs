//! Integration test: Uniswap V4 quote matches official V4 Quoter on Ethereum mainnet.
//!
//! Pool: USDC/WETH 0.05% (fee=500, tickSpacing=10, hooks=0x0).
//! PoolManager: 0x000000000004444c5dc75cB358380D2e3dE08A90
//! Set ETHEREUM_RPC_URL or RPC_URL to run.

use alloy::primitives::{Address, FixedBytes, Uint, keccak256};
use alloy::sol;
use alloy::sol_types::SolValue;
use std::str::FromStr;

use quote::abi::uniswap_v4_quote_single::PoolKey;
use quote::provider::create_provider;
use quote::uniswap_v4;

const POOL_MANAGER: &str = "0x000000000004444c5dc75cB358380D2e3dE08A90";
const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
const WETH: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
const QUOTER: &str = "0x52F0E24D1c21C8A0cB1e5a5dD6198556BD9E1203";
const ETHEREUM_UNI_V4_POSITION_MANAGER: &str = "0xbD216513d74C8cf14cf4747E6AaA6420FF64ee9ecargo";

/// Returns bytes32 poolId (keccak256(abi.encode(poolKey))); contract strips to bytes25 for PositionManager.
fn pool_id_bytes32(pool_key: &PoolKey) -> FixedBytes<32> {
    let encoded = pool_key.abi_encode();
    keccak256(&encoded)
}

/// USDC/WETH 0.05% pool: USDC (0xA0..) < WETH (0xC0..) so currency0=USDC, currency1=WETH
fn usdc_weth_pool_key() -> PoolKey {
    PoolKey {
        currency0: Address::from_str(USDC).expect("USDC"),
        currency1: Address::from_str(WETH).expect("WETH"),
        fee: Uint::from(500u32),
        tickSpacing: alloy::primitives::aliases::I24::try_from(10i32).unwrap(),
        hooks: Address::ZERO,
    }
}

sol! {
    #[sol(rpc)]
    interface IV4Quoter {
        struct PoolKey {
            address currency0;
            address currency1;
            uint24 fee;
            int24 tickSpacing;
            address hooks;
        }

        struct QuoteExactSingleParams {
            PoolKey poolKey;
            bool zeroForOne;
            uint128 exactAmount;
            bytes hookData;
        }

        function quoteExactInputSingle(QuoteExactSingleParams memory params)
            external
            returns (uint256 amountOut, uint256 gasEstimate);
    }
}

fn rpc_url() -> Option<String> {
    std::env::var("ETHEREUM_RPC_URL")
        .or_else(|_| std::env::var("RPC_URL"))
        .ok()
        .filter(|u| !u.is_empty())
}

async fn official_quoter_amount_out(
    provider: &quote::provider::MyProvider,
    pool_key: &PoolKey,
    zero_for_one: bool,
    amount_in: u128,
) -> u128 {
    let quoter = IV4Quoter::new(Address::from_str(QUOTER).unwrap(), provider);
    let params = IV4Quoter::QuoteExactSingleParams {
        poolKey: IV4Quoter::PoolKey {
            currency0: pool_key.currency0,
            currency1: pool_key.currency1,
            fee: pool_key.fee,
            tickSpacing: pool_key.tickSpacing,
            hooks: pool_key.hooks,
        },
        zeroForOne: zero_for_one,
        exactAmount: amount_in as u128,
        hookData: alloy::primitives::Bytes::new(),
    };
    quoter
        .quoteExactInputSingle(params)
        .call()
        .await
        .expect("Official V4 Quoter call failed")
        .amountOut
        .to::<u128>()
}

#[tokio::test]
async fn uniswap_v4_quote_matches_official_quoter_usdc_to_weth() {
    dotenv::dotenv().ok();
    let rpc_url = rpc_url().expect("ETHEREUM_RPC_URL or RPC_URL required for integration test");
    let provider = create_provider(&rpc_url).await.expect("provider");

    let pool_manager = Address::from_str(POOL_MANAGER).unwrap();
    let pool_key = usdc_weth_pool_key();
    let token_in = Address::from_str(USDC).unwrap();
    let amount_in = 1_000_000u128; // 1 USDC

    let quoted = uniswap_v4::quote(
        pool_manager,
        pool_key.clone(),
        token_in,
        amount_in,
        0,
        provider.clone(),
    )
    .await
    .expect("uniswap_v4::quote failed");

    let baseline = official_quoter_amount_out(&provider, &pool_key, true, amount_in).await;

    assert_eq!(
        quoted, baseline,
        "uniswap_v4::quote ({quoted}) != official V4 Quoter ({baseline})"
    );
}

#[tokio::test]
async fn uniswap_v4_quote_matches_official_quoter_weth_to_usdc() {
    dotenv::dotenv().ok();
    let rpc_url = rpc_url().expect("ETHEREUM_RPC_URL or RPC_URL required for integration test");
    let provider = create_provider(&rpc_url).await.expect("provider");

    let pool_manager = Address::from_str(POOL_MANAGER).unwrap();
    let pool_key = usdc_weth_pool_key();
    let token_in = Address::from_str(WETH).unwrap();
    let amount_in = 10u128.pow(18); // 1 WETH

    let quoted = uniswap_v4::quote(
        pool_manager,
        pool_key.clone(),
        token_in,
        amount_in,
        0,
        provider.clone(),
    )
    .await
    .expect("uniswap_v4::quote failed");

    let baseline = official_quoter_amount_out(&provider, &pool_key, false, amount_in).await;

    assert_eq!(
        quoted, baseline,
        "uniswap_v4::quote ({quoted}) != official V4 Quoter ({baseline})"
    );
}

#[tokio::test]
async fn uniswap_v4_quote_by_pool_id_matches_official_quoter_usdc_to_weth() {
    dotenv::dotenv().ok();
    let rpc_url = rpc_url().expect("ETHEREUM_RPC_URL or RPC_URL required for integration test");
    let provider = create_provider(&rpc_url).await.expect("provider");

    let pool_key = usdc_weth_pool_key();
    let pool_id = pool_id_bytes32(&pool_key);
    let position_manager = ETHEREUM_UNI_V4_POSITION_MANAGER;
    let token_in = Address::from_str(USDC).unwrap();
    let amount_in = 1_000_000u128; // 1 USDC

    let quoted = uniswap_v4::quote_by_pool_id(
        position_manager,
        pool_id,
        token_in,
        amount_in,
        0,
        provider.clone(),
    )
    .await
    .expect("uniswap_v4::quote_by_pool_id failed");

    let baseline = official_quoter_amount_out(&provider, &pool_key, true, amount_in).await;

    assert_eq!(
        quoted, baseline,
        "uniswap_v4::quote_by_pool_id ({quoted}) != official V4 Quoter ({baseline})"
    );
}
