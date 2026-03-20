use alloy::primitives::Bytes;
use alloy::{primitives::Address, transports::http::reqwest::Url};
use clap::error::{ContextKind, ContextValue};
use clap::{Parser, ValueHint};
use log::{debug, warn};
use std::str::FromStr;

use quote::quote;
use quote::types::Protocol;

#[derive(Debug, Parser)]
#[command(name = "quote")]
#[command(about = "Quote swap through DEX pool CLI")]
struct Args {
    /// Pool identifier:
    ///   V2/V3: 20-byte address hex (0x...)
    ///   V4:    32-byte poolId hex
    pool_id: String,
    #[arg(value_enum)]
    protocol: Protocol,
    /// Token in address
    token_in: String,
    /// Amount in (wei/smallest unit)
    amount_in: String,
    #[arg(value_hint = ValueHint::Url)]
    rpc_url: String,

    #[arg(long)]
    position_manager: Option<Address>,
}

fn parse_pool_id_bytes(s: &str) -> Result<Bytes, clap::Error> {
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
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

    let amount_out = quote(
        pool_id,
        args.protocol,
        token_in,
        amount_in,
        &args.rpc_url,
        args.position_manager,
    )
    .await
    .map_err(|e| {
        warn!("Quote failed: {e}");
        let mut err = clap::Error::new(clap::error::ErrorKind::Io);
        err.insert(ContextKind::Custom, ContextValue::String(e.to_string()));
        err
    })?;

    println!("{amount_out}");
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
