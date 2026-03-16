pub mod abi;
pub mod provider;
pub mod types;
pub mod uniswap_v2;

use types::Protocol;

pub const DEFAULT_UNISWAP_V2_FEE_BPS: u128 = 30;

/// Quote a swap: returns amount_out for given pool, token_in, token_out, amount_in.
pub async fn quote(
    pool_id: alloy::primitives::Address,
    protocol: Protocol,
    token_in: alloy::primitives::Address,
    amount_in: u128,
    rpc_url: &str,
) -> Result<u128, Box<dyn std::error::Error>> {
    let provider = provider::create_provider(rpc_url).await?;
    match protocol {
        Protocol::UniswapV2 => {
            uniswap_v2::quote(
                pool_id,
                token_in,
                amount_in,
                DEFAULT_UNISWAP_V2_FEE_BPS,
                provider,
            )
            .await
        }
        _ => Err(format!("Protocol {:?} not implemented", protocol).into()),
    }
}
