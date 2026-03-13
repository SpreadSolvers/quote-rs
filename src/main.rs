use std::str::FromStr;
use std::time::Instant;

use alloy::{primitives::Address, transports::http::reqwest::Url};
use clap::error::{ContextKind, ContextValue};
use clap::{Parser, ValueHint};
use log::{debug, warn};

use quote::provider::create_provider;
use quote::types::Protocol;

#[derive(Debug, Parser)]
#[command(name = "quote")]
#[command(about = "Quote swap through pool CLI")]
struct Args {
    pool_id: String,
    #[arg(value_enum)]
    protocol: Protocol,
    #[arg(value_hint = ValueHint::Url)]
    rpc_url: String,
}

async fn run(args: Args) -> Result<(), clap::Error> {
    let time = Instant::now();

    debug!("Args: {args:?}");

    let Ok(pool_id) = Address::from_str(&args.pool_id) else {
        return Err(clap::Error::new(clap::error::ErrorKind::InvalidValue));
    };

    debug!("Pool ID: {pool_id:?}");

    let Ok(rpc_url) = Url::parse(&args.rpc_url) else {
        warn!("Invalid RPC URL: {}", args.rpc_url);

        let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);

        err.insert(
            ContextKind::InvalidValue,
            ContextValue::String(format!("Invalid RPC URL: {}", args.rpc_url.clone())),
        );

        return Err(err);
    };

    debug!("Protocol: {:?}", args.protocol);

    debug!("RPC URL: {rpc_url:?}");

    debug!(
        "Getting pool data for pool_id: {} and rpc_url: {}",
        args.pool_id, args.rpc_url
    );

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
            let pool_data = uniswap_v2::quote(pool_id, provider.clone())
                .await
                .map_err(|e| {
                    warn!("Failed to parse pool data: {e}");
                    clap::Error::new(clap::error::ErrorKind::Io)
                })?;

            println!(
                "{}",
                serde_json::to_string_pretty(&pool_data).expect("Failed to serialize pool data")
            );
        }
        _ => {
            warn!("Not implemented protocol: {:?}", args.protocol);
            let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
            err.insert(
                ContextKind::InvalidValue,
                ContextValue::String(format!("Not implemented yet protocol: {:?}", args.protocol)),
            );

            return Err(err);
        }
    }

    debug!("Time taken: {:?}", time.elapsed());

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    dotenv::dotenv().ok();

    let args = Args::parse();
    if let Err(e) = run(args).await {
        // debug!("{:?}", e);

        eprintln!("CLI failed with the following errors:");

        for context in e.context() {
            eprintln!(
                "Reason: {:?}, Details: {}",
                context.0.to_string(),
                context.1.to_string()
            );
        }
    }
}
