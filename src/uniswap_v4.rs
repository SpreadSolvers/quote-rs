use alloy::contract::Error as ContractError;
use alloy::primitives::{Address, Bytes, U256};
use alloy::sol_types::{SolError, SolValue};

use crate::abi::uniswap_v4_quote_single::{
    PoolKey, QuoterConstructorArgs, UniswapV4QuoteSingle, V4PoolId,
};
use crate::abi::uniswap_v4_quote_single::UniswapV4QuoteSingle::{AmountOut, InsufficientLiquidity, PoolNotInitialized};
use crate::provider::MyProvider;

/// Encodes a V4 pool identifier as ABI-encoded bytes for use as `pool_id`.
pub fn encode_pool_id(pool_manager: Address, pool_key: &PoolKey) -> Bytes {
    V4PoolId {
        poolManager: pool_manager,
        currency0: pool_key.currency0,
        currency1: pool_key.currency1,
        fee: pool_key.fee,
        tickSpacing: pool_key.tickSpacing,
        hooks: pool_key.hooks,
    }
    .abi_encode()
    .into()
}

/// Decodes a V4 pool identifier from ABI-encoded bytes (as produced by `encode_pool_id`).
pub fn decode_pool_id(bytes: &[u8]) -> Result<(Address, PoolKey), Box<dyn std::error::Error>> {
    let v = V4PoolId::abi_decode(bytes)
        .map_err(|e| format!("Failed to decode V4 pool_id: {e}"))?;
    let pool_key = PoolKey {
        currency0: v.currency0,
        currency1: v.currency1,
        fee: v.fee,
        tickSpacing: v.tickSpacing,
        hooks: v.hooks,
    };
    Ok((v.poolManager, pool_key))
}

/// Returns deployment calldata for the V4 quoter (bytecode + encoded constructor args).
/// Used to run the quoter on a revm instance via CREATE tx.
pub fn quoter_deployment_data(
    pool_manager: Address,
    pool_key: PoolKey,
    token_in: Address,
    amount_in: u128,
    protocol_fee_bps: u128,
) -> Bytes {
    let bytecode = UniswapV4QuoteSingle::BYTECODE.as_ref().to_vec();
    let args = QuoterConstructorArgs {
        poolManager: pool_manager,
        key: pool_key,
        tokenIn: token_in,
        amountIn: U256::from(amount_in),
        protocolFeeBps: U256::from(protocol_fee_bps),
    };
    let encoded = args.abi_encode();
    let mut out = bytecode;
    out.extend(encoded);
    Bytes::from(out)
}

/// Extracts revert data from contract error.
fn revert_data_from_error(e: &ContractError) -> Option<Bytes> {
    if let Some(data) = e.as_revert_data() {
        return Some(data);
    }
    let ContractError::TransportError(te) = e else {
        return None;
    };
    let payload = te.as_error_resp()?;
    let raw = payload.data.as_ref()?;
    let s = raw.get().trim_matches('"').trim();
    let hex_str = s
        .strip_prefix("Reverted 0x")
        .or_else(|| s.strip_prefix("0x"))?;
    hex::decode(hex_str).ok().map(Bytes::from)
}

pub async fn quote(
    pool_manager: Address,
    pool_key: PoolKey,
    token_in: Address,
    amount_in: u128,
    protocol_fee_bps: u128,
    provider: MyProvider,
) -> Result<u128, Box<dyn std::error::Error>> {
    let amount_in_u256 = U256::from(amount_in);

    let result: Result<Bytes, ContractError> = UniswapV4QuoteSingle::deploy_builder(
        provider.clone(),
        pool_manager,
        pool_key,
        token_in,
        amount_in_u256,
        U256::from(protocol_fee_bps),
    )
    .call()
    .await;

    let err = result
        .err()
        .ok_or("Ephemeral quoter returned success (unexpected)")?;

    let bytes =
        revert_data_from_error(&err).ok_or_else(|| format!("Could not decode revert: {err:?}"))?;

    if InsufficientLiquidity::abi_decode(bytes.as_ref()).is_ok() {
        return Err("pool has 0 active liquidity".into());
    }

    if PoolNotInitialized::abi_decode(bytes.as_ref()).is_ok() {
        return Err("pool not initialized".into());
    }

    let decoded = AmountOut::abi_decode(bytes.as_ref())
        .map_err(|_| format!("Could not decode revert: {err:?}"))?;

    Ok(decoded.amountOut.to::<u128>())
}
