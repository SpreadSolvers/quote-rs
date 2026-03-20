use alloy::primitives::Bytes;
use alloy::{primitives::Address, transports::http::reqwest::Url};
use clap::error::{ContextKind, ContextValue};
use clap::{Parser, ValueHint};
use log::{debug, warn};
use std::str::FromStr;

use quote::provider::create_provider;
use quote::types::Protocol;
use quote::{
    uniswap_v2, uniswap_v3, uniswap_v4, DEFAULT_UNISWAP_V2_FEE_BPS, DEFAULT_UNISWAP_V3_FEE_BPS,
    DEFAULT_UNISWAP_V4_FEE_BPS,
};

#[derive(Debug, Parser)]
#[command(name = "quote")]
#[command(about = "Quote swap through DEX pool CLI")]
struct Args {
    /// Pool identifier:
    ///   V2/V3: 20-byte address hex (0x...)
    ///   V4:    hex-encoded ABI bytes from uniswap_v4::encode_pool_id (poolManager + PoolKey)
    pool_id: String,
    #[arg(value_enum)]
    protocol: Protocol,
    /// Token in address
    token_in: String,
    /// Amount in (wei/smallest unit)
    amount_in: String,
    #[arg(value_hint = ValueHint::Url)]
    rpc_url: String,
}

fn parse_pool_id_bytes(s: &str) -> Result<Bytes, clap::Error> {
    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
    let raw = hex::decode(hex).map_err(|_| {
        let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
        err.insert(
            ContextKind::InvalidValue,
            ContextValue::String("Invalid pool_id hex".to_string()),
        );
        err
    })?;
    Ok(Bytes::from(raw))
}

async fn run(args: Args) -> Result<(), clap::Error> {
    debug!("Args: {args:?}");

    let pool_id = parse_pool_id_bytes(&args.pool_id)?;

    let Ok(token_in) = Address::from_str(&args.token_in) else {
        let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
        err.insert(
            ContextKind::InvalidValue,
            ContextValue::String("Invalid token_in address".to_string()),
        );
        return Err(err);
    };

    let Ok(amount_in) = args.amount_in.parse::<u128>() else {
        let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
        err.insert(
            ContextKind::InvalidValue,
            ContextValue::String("Invalid amount_in (expected uint128)".to_string()),
        );
        return Err(err);
    };

    let Ok(_rpc_url) = Url::parse(&args.rpc_url) else {
        let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
        err.insert(
            ContextKind::InvalidValue,
            ContextValue::String(format!("Invalid RPC URL: {}", args.rpc_url)),
        );
        return Err(err);
    };

    let provider = create_provider(&args.rpc_url).await.map_err(|e| {
        warn!("Failed to create provider: {e}");
        let mut err = clap::Error::new(clap::error::ErrorKind::Io);
        err.insert(
            ContextKind::Custom,
            ContextValue::String("Failed to create Web3 provider".to_string()),
        );
        err.insert(ContextKind::Custom, ContextValue::String(e.to_string()));
        err
    })?;

    let amount_out = match args.protocol {
        Protocol::UniswapV2 => {
            let addr = alloy::primitives::Address::try_from(pool_id.as_ref()).map_err(|_| {
                let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
                err.insert(
                    ContextKind::InvalidValue,
                    ContextValue::String("V2 pool_id must be a 20-byte address".to_string()),
                );
                err
            })?;
            uniswap_v2::quote(addr, token_in, amount_in, DEFAULT_UNISWAP_V2_FEE_BPS, provider)
                .await
        }
        Protocol::UniswapV3 => {
            let addr = alloy::primitives::Address::try_from(pool_id.as_ref()).map_err(|_| {
                let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
                err.insert(
                    ContextKind::InvalidValue,
                    ContextValue::String("V3 pool_id must be a 20-byte address".to_string()),
                );
                err
            })?;
            uniswap_v3::quote(addr, token_in, amount_in, DEFAULT_UNISWAP_V3_FEE_BPS, provider)
                .await
        }
        Protocol::UniswapV4 => {
            let (pool_manager, pool_key) =
                uniswap_v4::decode_pool_id(&pool_id).map_err(|e| {
                    warn!("Failed to decode V4 pool_id: {e}");
                    let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
                    err.insert(
                        ContextKind::InvalidValue,
                        ContextValue::String(format!("Invalid V4 pool_id: {e}")),
                    );
                    err
                })?;
            uniswap_v4::quote(
                pool_manager,
                pool_key,
                token_in,
                amount_in,
                DEFAULT_UNISWAP_V4_FEE_BPS,
                provider,
            )
            .await
        }
        _ => {
            warn!("Not implemented protocol: {:?}", args.protocol);
            let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
            err.insert(
                ContextKind::InvalidValue,
                ContextValue::String(format!("Protocol not implemented: {:?}", args.protocol)),
            );
            return Err(err);
        }
    };

    match amount_out {
        Ok(out) => println!("{out}"),
        Err(e) => {
            warn!("Failed to quote: {e}");
            let mut err = clap::Error::new(clap::error::ErrorKind::Io);
            err.insert(ContextKind::Custom, ContextValue::String(e.to_string()));
            return Err(err);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();
    dotenv::dotenv().ok();

    let args = Args::parse();
    if let Err(e) = run(args).await {
        eprintln!("Error:");
        for context in e.context() {
            eprintln!("  {}: {}", context.0.to_string(), context.1.to_string());
        }
        std::process::exit(1);
    }
}
