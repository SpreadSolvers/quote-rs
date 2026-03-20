pub mod abi;
pub mod provider;
pub mod types;
pub mod uniswap_v2;
pub mod uniswap_v3;
pub mod uniswap_v4;

use types::Protocol;

pub const DEFAULT_UNISWAP_V2_FEE_BPS: u128 = 30;
pub const DEFAULT_UNISWAP_V3_FEE_BPS: u128 = 0;
pub const DEFAULT_UNISWAP_V4_FEE_BPS: u128 = 0;

/// Quote a swap: returns amount_out for given pool, token_in, token_out, amount_in.
///
/// `pool_id` encoding:
/// - V2 / V3: raw 20-byte pool address
/// - V4: ABI-encoded `V4PoolId` struct (poolManager + PoolKey fields), see `uniswap_v4::encode_pool_id`
pub async fn quote(
    pool_id: alloy::primitives::Bytes,
    protocol: Protocol,
    token_in: alloy::primitives::Address,
    amount_in: u128,
    rpc_url: &str,
) -> Result<u128, Box<dyn std::error::Error>> {
    let provider = provider::create_provider(rpc_url).await?;
    match protocol {
        Protocol::UniswapV2 => {
            let addr = alloy::primitives::Address::try_from(pool_id.as_ref())
                .map_err(|_| "V2 pool_id must be a 20-byte address")?;
            uniswap_v2::quote(addr, token_in, amount_in, DEFAULT_UNISWAP_V2_FEE_BPS, provider)
                .await
        }
        Protocol::UniswapV3 => {
            let addr = alloy::primitives::Address::try_from(pool_id.as_ref())
                .map_err(|_| "V3 pool_id must be a 20-byte address")?;
            uniswap_v3::quote(addr, token_in, amount_in, DEFAULT_UNISWAP_V3_FEE_BPS, provider)
                .await
        }
        Protocol::UniswapV4 => {
            let (pool_manager, pool_key) = uniswap_v4::decode_pool_id(&pool_id)?;
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
        _ => Err(format!("Protocol {:?} not implemented", protocol).into()),
    }
}
