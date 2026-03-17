//! Example: Cross-chain arbitrage between Ethereum and Fuse WBTC/WETH pools.
//!
//! Quotes both pools, finds price difference, optimizes amount and direction.
//! Run with: cargo run --example arb_wbtc_weth
//!
//! Env vars: ETHEREUM_RPC_URL (or RPC_URL), FUSE_RPC_URL (or RPC_URL)

use alloy::primitives::Address;
use quote::provider::create_provider;
use quote::{DEFAULT_UNISWAP_V2_FEE_BPS, uniswap_v2};
use std::str::FromStr;

// Ethereum Uniswap V2 WBTC/WETH
const ETH_POOL: &str = "0xbb2b8038a1640196fbe3e38816f3e67cba72d940";
const ETH_WBTC: &str = "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599";
const ETH_WETH: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";

// Fuse Voltage WBTC/WETH
const FUSE_POOL: &str = "0x97f4f45f0172f2e20ab284a61c8adcf5e4d04228";
const FUSE_WBTC: &str = "0x33284f95ccb7b948d9d352e1439561cf83d8d00d";
const FUSE_WETH: &str = "0xa722c13135930332eb3d749b2f0906559d2c5b99";

const WBTC_DECIMALS: u32 = 8;
const WETH_DECIMALS: u32 = 18;

const STEPS: u32 = 10;
const REF_WETH_AMOUNT: u128 = 10u128.pow(WETH_DECIMALS) / 100;
const MAX_WETH_AMOUNT_MULTIPLIER: u128 = 100;
const MIN_WETH_AMOUNT_MULTIPLIER: u128 = 100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    dotenv::dotenv().ok();

    let eth_rpc = std::env::var("ETHEREUM_RPC_URL")
        .or_else(|_| std::env::var("RPC_URL"))
        .unwrap_or_else(|_| {
            eprintln!("Set ETHEREUM_RPC_URL (or RPC_URL) and FUSE_RPC_URL (or RPC_URL)");
            std::process::exit(1);
        });
    let fuse_rpc = std::env::var("FUSE_RPC_URL")
        .or_else(|_| std::env::var("RPC_URL"))
        .unwrap_or_else(|_| {
            eprintln!("Set ETHEREUM_RPC_URL (or RPC_URL) and FUSE_RPC_URL (or RPC_URL)");
            std::process::exit(1);
        });

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run(eth_rpc, fuse_rpc))
}

async fn run(eth_rpc: String, fuse_rpc: String) -> Result<(), Box<dyn std::error::Error>> {
    let eth_pool = Address::from_str(ETH_POOL)?;
    let fuse_pool = Address::from_str(FUSE_POOL)?;
    let eth_wbtc = Address::from_str(ETH_WBTC)?;
    let eth_weth = Address::from_str(ETH_WETH)?;
    let fuse_wbtc = Address::from_str(FUSE_WBTC)?;
    let fuse_weth = Address::from_str(FUSE_WETH)?;

    let eth_provider = create_provider(&eth_rpc).await?;
    let fuse_provider = create_provider(&fuse_rpc).await?;

    // Reference amount: 0.01 WETH (18 decimals)
    let ref_weth: u128 = REF_WETH_AMOUNT;

    println!("=== WBTC/WETH cross-chain arbitrage ===\n");
    println!("Reference: {ref_weth} wei WETH (~0.01 WETH)\n");

    // Quote WETH -> WBTC on both chains
    let eth_weth_to_wbtc = uniswap_v2::quote(
        eth_pool,
        eth_weth,
        ref_weth,
        DEFAULT_UNISWAP_V2_FEE_BPS,
        eth_provider.clone(),
    )
    .await?;
    let fuse_weth_to_wbtc = uniswap_v2::quote(
        fuse_pool,
        fuse_weth,
        ref_weth,
        DEFAULT_UNISWAP_V2_FEE_BPS,
        fuse_provider.clone(),
    )
    .await?;

    // Price = WBTC per 1 WETH (scaled)
    let eth_price_wbtc_per_weth = (eth_weth_to_wbtc as u128) * 10u128.pow(WETH_DECIMALS) / ref_weth;
    let fuse_price_wbtc_per_weth =
        (fuse_weth_to_wbtc as u128) * 10u128.pow(WETH_DECIMALS) / ref_weth;

    println!(
        "Ethereum:  {ref_weth} WETH -> {eth_weth_to_wbtc} WBTC  (price: {eth_price_wbtc_per_weth} WBTC/1e18 WETH)"
    );
    println!(
        "Fuse:      {ref_weth} WETH -> {fuse_weth_to_wbtc} WBTC  (price: {fuse_price_wbtc_per_weth} WBTC/1e18 WETH)"
    );

    let spread_bps = if eth_price_wbtc_per_weth > fuse_price_wbtc_per_weth {
        ((eth_price_wbtc_per_weth - fuse_price_wbtc_per_weth) as i128) * 10_000
            / fuse_price_wbtc_per_weth as i128
    } else {
        ((fuse_price_wbtc_per_weth - eth_price_wbtc_per_weth) as i128) * 10_000
            / eth_price_wbtc_per_weth as i128
    };
    println!("\nPrice spread: {} bps", spread_bps);

    // Determine direction: higher WBTC-per-WETH = WBTC cheaper there.
    // Buy WBTC where it's cheaper (more WBTC per WETH), sell where it's dearer (less WBTC per WETH).
    let (buy_chain, sell_chain, buy_pool, sell_pool, buy_weth, _buy_wbtc, _sell_weth, sell_wbtc) =
        if eth_price_wbtc_per_weth > fuse_price_wbtc_per_weth {
            // Eth gives more WBTC per WETH → WBTC cheaper on Eth → buy Eth, sell Fuse
            (
                "Ethereum", "Fuse", eth_pool, fuse_pool, eth_weth, eth_wbtc, fuse_weth, fuse_wbtc,
            )
        } else {
            // Fuse gives more WBTC per WETH → WBTC cheaper on Fuse → buy Fuse, sell Eth
            (
                "Fuse", "Ethereum", fuse_pool, eth_pool, fuse_weth, fuse_wbtc, eth_weth, eth_wbtc,
            )
        };

    println!("\nProfitable direction: buy WBTC on {buy_chain}, sell on {sell_chain}");

    // Optimize amount: grid search over WETH amounts
    let providers = if buy_chain == "Fuse" {
        (fuse_provider.clone(), eth_provider.clone())
    } else {
        (eth_provider.clone(), fuse_provider.clone())
    };

    let min_weth = ref_weth / MIN_WETH_AMOUNT_MULTIPLIER;
    let max_weth = ref_weth * MAX_WETH_AMOUNT_MULTIPLIER;
    let step = (max_weth - min_weth) / STEPS.max(1) as u128;

    let mut best_profit: i128 = i128::MIN;
    let mut best_amount_in: u128 = min_weth;
    let mut best_wbtc_out: u128 = 0;
    let mut best_weth_back: u128 = 0;

    for i in 0..=STEPS {
        let amount_weth = min_weth + step * i as u128;
        let wbtc_out = uniswap_v2::quote(
            buy_pool,
            buy_weth,
            amount_weth,
            DEFAULT_UNISWAP_V2_FEE_BPS,
            providers.0.clone(),
        )
        .await
        .unwrap_or(0);
        if wbtc_out == 0 {
            continue;
        }
        let weth_back = uniswap_v2::quote(
            sell_pool,
            sell_wbtc,
            wbtc_out,
            DEFAULT_UNISWAP_V2_FEE_BPS,
            providers.1.clone(),
        )
        .await
        .unwrap_or(0);
        let profit = weth_back as i128 - amount_weth as i128;
        if profit > best_profit {
            best_profit = profit;
            best_amount_in = amount_weth;
            best_wbtc_out = wbtc_out;
            best_weth_back = weth_back;
        }
    }

    println!("\n--- Optimized arbitrage ---");
    println!(
        "Amount in:  {} wei WETH ({:.6} WETH)",
        best_amount_in,
        best_amount_in as f64 / 10f64.powi(WETH_DECIMALS as i32)
    );
    println!(
        "WBTC out:   {} wei ({:.8} WBTC)",
        best_wbtc_out,
        best_wbtc_out as f64 / 10f64.powi(WBTC_DECIMALS as i32)
    );
    println!(
        "WETH back:  {} wei ({:.6} WETH)",
        best_weth_back,
        best_weth_back as f64 / 10f64.powi(WETH_DECIMALS as i32)
    );
    println!(
        "Profit:     {} wei ({:.6} WETH)",
        best_profit,
        best_profit as f64 / 10f64.powi(WETH_DECIMALS as i32)
    );
    if best_profit > 0 {
        let roi_bps = (best_profit as i128) * 10_000 / best_amount_in as i128;
        println!("ROI:        {} bps", roi_bps);
    } else {
        println!("(No profitable amount found in range; spread may not cover fees/slippage)");
    }

    Ok(())
}
