use std::str::FromStr;

use alloy::{primitives::Address, transports::http::reqwest::Url};
use clap::error::{ContextKind, ContextValue};
use clap::{Parser, ValueHint};
use log::{debug, warn};

use quote::provider::create_provider;
use quote::types::Protocol;
use quote::{DEFAULT_UNISWAP_V2_FEE_BPS, DEFAULT_UNISWAP_V3_FEE_BPS, uniswap_v2, uniswap_v3};

#[derive(Debug, Parser)]
#[command(name = "quote")]
#[command(about = "Quote swap through DEX pool CLI")]
struct Args {
    /// Pool address (pair for UniswapV2)
    pool_id: String,
    #[arg(value_enum)]
    protocol: Protocol,
    /// Token in address
    token_in: String,
    // /// Token out address
    // token_out: String,
    /// Amount in (wei/smallest unit)
    amount_in: String,
    #[arg(value_hint = ValueHint::Url)]
    rpc_url: String,
}

async fn run(args: Args) -> Result<(), clap::Error> {
    debug!("Args: {args:?}");

    let Ok(pool_id) = Address::from_str(&args.pool_id) else {
        let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
        err.insert(
            ContextKind::InvalidValue,
            ContextValue::String("Invalid pool_id address".to_string()),
        );
        return Err(err);
    };

    let Ok(token_in) = Address::from_str(&args.token_in) else {
        let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
        err.insert(
            ContextKind::InvalidValue,
            ContextValue::String("Invalid token_in address".to_string()),
        );
        return Err(err);
    };

    // let Ok(token_out) = Address::from_str(&args.token_out) else {
    //     let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
    //     err.insert(
    //         ContextKind::InvalidValue,
    //         ContextValue::String("Invalid token_out address".to_string()),
    //     );
    //     return Err(err);
    // };

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

    match args.protocol {
        Protocol::UniswapV2 => {
            let amount_out = uniswap_v2::quote(
                pool_id,
                token_in,
                amount_in,
                DEFAULT_UNISWAP_V2_FEE_BPS,
                provider,
            )
            .await
            .map_err(|e| {
                warn!("Failed to quote: {e}");
                let mut err = clap::Error::new(clap::error::ErrorKind::Io);
                err.insert(ContextKind::Custom, ContextValue::String(e.to_string()));
                err
            })?;
            println!("{amount_out}");
        }
        Protocol::UniswapV3 => {
            let amount_out = uniswap_v3::quote(
                pool_id,
                token_in,
                amount_in,
                DEFAULT_UNISWAP_V3_FEE_BPS,
                provider,
            )
            .await
            .map_err(|e| {
                warn!("Failed to quote: {e}");
                let mut err = clap::Error::new(clap::error::ErrorKind::Io);
                err.insert(ContextKind::Custom, ContextValue::String(e.to_string()));
                err
            })?;
            println!("{amount_out}");
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
