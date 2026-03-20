pub mod abi;
pub mod provider;
pub mod types;
pub mod uniswap_v2;
pub mod uniswap_v3;
pub mod uniswap_v4;
mod utils;

use alloy::primitives::FixedBytes;
use types::Protocol;

pub const DEFAULT_UNISWAP_V2_FEE_BPS: u128 = 30;
pub const DEFAULT_UNISWAP_V3_FEE_BPS: u128 = 0;
pub const DEFAULT_UNISWAP_V4_FEE_BPS: u128 = 0;

/// Quote a swap: returns amount_out for given pool, token_in, token_out, amount_in.
///
/// `pool_id` encoding:
/// - V2 / V3: raw 20-byte pool address
/// - V4: 32-byte poolId (requires address of PositionManager to be provided)
pub async fn quote(
    pool_id: alloy::primitives::Bytes,
    protocol: Protocol,
    token_in: alloy::primitives::Address,
    amount_in: u128,
    rpc_url: &str,
    position_manager: Option<alloy::primitives::Address>,
) -> Result<u128, Box<dyn std::error::Error>> {
    let provider = provider::create_provider(rpc_url).await?;
    match protocol {
        Protocol::UniswapV2 => {
            let addr = alloy::primitives::Address::try_from(pool_id.as_ref())
                .map_err(|_| "V2 pool_id must be a 20-byte address")?;
            uniswap_v2::quote(
                addr,
                token_in,
                amount_in,
                DEFAULT_UNISWAP_V2_FEE_BPS,
                provider,
            )
            .await
        }
        Protocol::UniswapV3 => {
            let addr = alloy::primitives::Address::try_from(pool_id.as_ref())
                .map_err(|_| "V3 pool_id must be a 20-byte address")?;
            uniswap_v3::quote(
                addr,
                token_in,
                amount_in,
                DEFAULT_UNISWAP_V3_FEE_BPS,
                provider,
            )
            .await
        }
        Protocol::UniswapV4 => {
            if let Some(position_manager) = position_manager {
                let pool_id_fb: FixedBytes<32> = pool_id
                    .as_ref()
                    .try_into()
                    .map_err(|_| "V4 pool_id must be 32 bytes (bytes32 poolId)")?;
                uniswap_v4::quote_by_pool_id(
                    position_manager,
                    pool_id_fb,
                    token_in,
                    amount_in,
                    DEFAULT_UNISWAP_V4_FEE_BPS,
                    provider,
                )
                .await
            } else {
                Err(format!("Position manager is required for V4 quote").into())
            }
        }
        _ => Err(format!("Protocol {:?} not implemented", protocol).into()),
    }
}
