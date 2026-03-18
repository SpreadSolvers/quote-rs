//! Integration test: Uniswap V3 quote matches QuoterV2 on Ethereum mainnet.
//!
//! Uses revm fork to run both our quoter (CREATE) and QuoterV2 (call).
//! Pool: USDC/WETH 0.3% `0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8`.
//! Set ETHEREUM_RPC_URL or RPC_URL to run. Fork block 21_500_000 for caching.

use alloy::consensus::BlockHeader;
use alloy::eips::BlockId;
use alloy::primitives::{Address, B256, U256, Uint};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::sol;
use alloy::sol_types::{SolCall, SolError, SolValue};
use quote::abi::uniswap_v3_quote_single::UniswapV3QuoteSingle::AmountOut;
use quote::uniswap_v3::quoter_deployment_data;
use revm::context::{BlockEnv, CfgEnv, TxEnv};
use revm::context_interface::result::ExecutionResult;
use revm::database::{CacheDB, WrapDatabaseRef};
use revm::primitives::{TxKind, hardfork::SpecId};
use revm::state::AccountInfo;
use revm::{Context, ExecuteEvm, MainBuilder, MainContext};
use std::str::FromStr;
use std::sync::Arc;

use foundry_fork_db::{BlockchainDb, SharedBackend, cache::BlockchainDbMeta};

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

const ETH_FORK_BLOCK: u64 = 21_500_000;
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

fn decode_quoter_v2_result(result: &ExecutionResult) -> u128 {
    match result {
        ExecutionResult::Success { output, .. } => {
            let (amount_out, _, _, _) =
                <(U256, alloy::primitives::U160, u32, U256)>::abi_decode(output.data())
                    .expect("decode QuoterV2 return");
            amount_out.to::<u128>()
        }
        ExecutionResult::Revert { output, .. } => {
            assert_eq!(output.len(), 96, "QuoterV2 revert should be 96 bytes");
            let (amount_out, _, _) =
                <(U256, alloy::primitives::U160, i32)>::abi_decode(output.as_ref())
                    .expect("decode QuoterV2 revert");
            amount_out.to::<u128>()
        }
        other => panic!("QuoterV2 unexpected result: {other:?}"),
    }
}

async fn assert_quote_matches(
    pool_id: Address,
    token_in: Address,
    token_out: Address,
    amount_in: u128,
) {
    let rpc_url = rpc_url().expect("ETHEREUM_RPC_URL or RPC_URL required for integration test");
    dotenv::dotenv().ok();
    let _ = env_logger::try_init();

    let provider = ProviderBuilder::new()
        .network::<alloy::network::AnyNetwork>()
        .connect_http(rpc_url.parse().expect("rpc url"));

    let block = provider
        .get_block(BlockId::number(ETH_FORK_BLOCK))
        .await
        .expect("get_block")
        .expect("block exists");

    let chain_id = provider.get_chain_id().await.expect("chain_id");
    let basefee = block.header.base_fee_per_gas().unwrap_or_default();
    let prevrandao = block
        .header
        .mix_hash()
        .filter(|h| *h != B256::ZERO)
        .unwrap_or_else(|| B256::repeat_byte(0x01));

    let meta = BlockchainDbMeta::default()
        .with_block(&block.inner)
        .with_url(&rpc_url);
    let db = BlockchainDb::new(meta, None);
    let shared = SharedBackend::spawn_backend(Arc::new(provider), db, None).await;

    let caller = Address::from([0x11u8; 20]);
    let block_env = BlockEnv {
        number: U256::from(block.header.number()),
        beneficiary: block.header.beneficiary(),
        timestamp: U256::from(block.header.timestamp()),
        gas_limit: block.header.gas_limit(),
        basefee,
        prevrandao: Some(prevrandao),
        difficulty: block.header.difficulty(),
        ..Default::default()
    };

    let mut cache_db = CacheDB::new(WrapDatabaseRef(shared));
    cache_db.insert_account_info(
        caller,
        AccountInfo {
            balance: U256::from(10u128.pow(20)),
            ..Default::default()
        },
    );

    let mut evm = Context::mainnet()
        .with_block(block_env)
        .with_db(cache_db)
        .with_cfg(CfgEnv::new_with_spec(SpecId::CANCUN).with_chain_id(chain_id))
        .modify_cfg_chained(|c| c.disable_nonce_check = true)
        .build_mainnet();

    let quoter_v2_addr = Address::from_str(QUOTER_V2).unwrap();

    // Our quoter
    let deploy_data = quoter_deployment_data(pool_id, token_in, amount_in, 0);
    let quote_res = evm
        .transact(TxEnv {
            caller,
            kind: TxKind::Create,
            value: U256::ZERO,
            data: deploy_data.into(),
            gas_limit: 5_000_000,
            gas_price: basefee as u128,
            chain_id: Some(chain_id),
            ..Default::default()
        })
        .expect("quoter transact");

    let our_quote = match &quote_res.result {
        ExecutionResult::Revert { output, .. } => AmountOut::abi_decode(output.as_ref())
            .expect("decode AmountOut")
            .amountOut
            .to::<u128>(),
        _ => panic!("quoter expected to revert with AmountOut"),
    };

    // QuoterV2 baseline
    let params = IQuoterV2::QuoteExactInputSingleParams {
        tokenIn: token_in,
        tokenOut: token_out,
        amountIn: U256::from(amount_in),
        fee: Uint::<24, 1>::from(3000),
        sqrtPriceLimitX96: alloy::primitives::U160::ZERO,
    };
    let quoter_res = evm
        .transact(TxEnv {
            caller,
            kind: TxKind::Call(quoter_v2_addr),
            value: U256::ZERO,
            data: IQuoterV2::quoteExactInputSingleCall { params }
                .abi_encode()
                .into(),
            gas_limit: 5_000_000,
            gas_price: basefee as u128,
            chain_id: Some(chain_id),
            ..Default::default()
        })
        .expect("QuoterV2 transact");

    let quoter_v2_amount_out = decode_quoter_v2_result(&quoter_res.result);

    assert_eq!(
        our_quote, quoter_v2_amount_out,
        "UniswapV3QuoteSingle ({our_quote}) != QuoterV2 ({quoter_v2_amount_out})"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn uniswap_v3_quote_matches_quoter_v2_usdc_to_weth() {
    dotenv::dotenv().ok();
    env_logger::try_init().ok();

    rpc_url().expect("ETHEREUM_RPC_URL or RPC_URL required for integration test");
    assert_quote_matches(
        Address::from_str(POOL).unwrap(),
        Address::from_str(USDC).unwrap(),
        Address::from_str(WETH).unwrap(),
        1_000_000, // 1 USDC
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn uniswap_v3_quote_matches_quoter_v2_weth_to_usdc() {
    dotenv::dotenv().ok();
    env_logger::try_init().ok();

    rpc_url().expect("ETHEREUM_RPC_URL or RPC_URL required for integration test");
    assert_quote_matches(
        Address::from_str(POOL).unwrap(),
        Address::from_str(WETH).unwrap(),
        Address::from_str(USDC).unwrap(),
        10u128.pow(18), // 1 WETH
    )
    .await;
}
