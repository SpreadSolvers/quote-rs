//! Integration test: Uniswap V2 quote matches an actual swap in `revm` on a Fuse fork.
//!
//! Pool (UniV2 fork): WBTC/WETH `0x97f4f45f0172f2e20ab284a61c8adcf5e4d04228`
//! Router: `0xE3F85aAd0c8DD7337427B9dF5d0fB741d65EEEB5`
//!
//! Required env vars:
//! - `FUSE_RPC_URL` (or `RPC_URL`)
//! - `FUSE_FORK_BLOCK` (fixed block number for caching + reproducibility)

use alloy::consensus::BlockHeader;
use alloy::eips::BlockId;
use alloy::network::AnyNetwork;
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::sol;
use alloy::sol_types::SolCall;
use log::debug;
use quote::provider::create_provider;
use quote::{DEFAULT_UNISWAP_V2_FEE_BPS, uniswap_v2};
use revm::Context;
use revm::ExecuteEvm;
use revm::context::{BlockEnv, TxEnv};
use revm::database::{CacheDB, WrapDatabaseRef};
use revm::primitives::TxKind;
use revm::state::AccountInfo;
use revm::{MainBuilder, MainContext};
use std::str::FromStr;

use foundry_fork_db::{BlockchainDb, SharedBackend, cache::BlockchainDbMeta};
use std::sync::Arc;

sol! {
    interface IUniswapV2Router02 {
        function swapExactETHForTokens(uint256 amountOutMin, address[] path, address to, uint256 deadline)
            external
            payable
            returns (uint256[] amounts);
    }
}

const FUSE_CHAIN_ID: u64 = 122;
const FUSE_FORK_BLOCK_NUMBER: u64 = 40967484;
const WBTC_WETH_POOL: &str = "0x97f4f45f0172f2e20ab284a61c8adcf5e4d04228";
const WETH: &str = "0xa722c13135930332eb3d749b2f0906559d2c5b99";
const WBTC: &str = "0x33284f95ccb7b948d9d352e1439561cf83d8d00d";
const ROUTER: &str = "0xE3F85aAd0c8DD7337427B9dF5d0fB741d65EEEB5";
/// 0.01 WETH (18 decimals) via `swapExactETHForTokens` (WETH wrapping happens inside the router).
const AMOUNT_IN_WEI: u128 = 10_000_000_000_000_000u128;

#[tokio::test]
async fn uniswap_v2_quote_matches_revm_swap_exact_eth_for_tokens_weth_to_wbtc() {
    let rpc_url = std::env::var("FUSE_RPC_URL").or_else(|_| std::env::var("RPC_URL"));
    let rpc_url = match rpc_url {
        Ok(u) if !u.is_empty() => u,
        _ => {
            assert!(false, "Skip: set FUSE_RPC_URL or RPC_URL to run");
            return;
        }
    };

    let pool_id = Address::from_str(WBTC_WETH_POOL).expect("WBTC_WETH_POOL");
    let token_in = Address::from_str(WETH).expect("WETH");
    let token_out = Address::from_str(WBTC).expect("WBTC");
    let router_addr = Address::from_str(ROUTER).expect("ROUTER");

    let provider = create_provider(&rpc_url).await.expect("provider");

    let quoted_amount_out = uniswap_v2::quote(
        pool_id,
        token_in,
        AMOUNT_IN_WEI,
        DEFAULT_UNISWAP_V2_FEE_BPS,
        provider.clone(),
    )
    .await
    .expect("quote");

    debug!("Quoted amount {:?}", quoted_amount_out);

    let fork_provider = ProviderBuilder::new()
        .network::<AnyNetwork>()
        .connect_http(rpc_url.parse().expect("rpc url"));

    let fork_block_id = BlockId::number(FUSE_FORK_BLOCK_NUMBER);
    let block = fork_provider
        .get_block(fork_block_id)
        .await
        .expect("get_block rpc")
        .expect("block exists");

    let meta = BlockchainDbMeta::default()
        .with_block(&block.inner)
        .with_url(&rpc_url);
    let db = BlockchainDb::new(meta, None);
    let shared = SharedBackend::spawn_backend(Arc::new(fork_provider), db, None).await;

    let caller = Address::from([0x11u8; 20]);

    let block_env = BlockEnv {
        number: U256::from(block.header.number()),
        beneficiary: block.header.beneficiary(),
        timestamp: U256::from(block.header.timestamp()),
        gas_limit: block.header.gas_limit(),
        basefee: block.header.base_fee_per_gas().unwrap_or_default(),
        prevrandao: block.header.mix_hash(),
        difficulty: block.header.difficulty(),
        ..Default::default()
    };

    let mut cache_db = CacheDB::new(WrapDatabaseRef(shared));
    cache_db.insert_account_info(
        caller,
        AccountInfo {
            balance: U256::from(10u128.pow(20)), // plenty of ETH for msg.value + gas
            ..Default::default()
        },
    );

    let deadline = block.header.timestamp().saturating_add(60 * 60);
    let path = vec![token_in, token_out];

    let calldata = IUniswapV2Router02::swapExactETHForTokensCall {
        amountOutMin: U256::ZERO,
        path,
        to: caller,
        deadline: U256::from(deadline),
    }
    .abi_encode();

    let tx = TxEnv {
        caller,
        kind: TxKind::Call(router_addr),
        value: U256::from(AMOUNT_IN_WEI),
        data: calldata.into(),
        gas_limit: 3_000_000,
        gas_price: block_env.basefee as u128,
        chain_id: Some(FUSE_CHAIN_ID),
        ..Default::default()
    };

    let mut evm = Context::mainnet()
        // .with_block(block_env)
        .with_db(cache_db)
        .build_mainnet();

    let res = evm.transact(tx).expect("revm transact");
    let output = res.result.output().expect("output bytes");
    let decoded_amounts: Vec<U256> =
        IUniswapV2Router02::swapExactETHForTokensCall::abi_decode_returns(output.as_ref())
            .expect("decode router return");
    let swap_amount_out_u128: u128 = decoded_amounts
        .last()
        .copied()
        .unwrap_or_default()
        .to::<u128>();

    assert_eq!(
        quoted_amount_out, swap_amount_out_u128,
        "quote ({quoted_amount_out}) != revm swap output ({swap_amount_out_u128})"
    );
}
