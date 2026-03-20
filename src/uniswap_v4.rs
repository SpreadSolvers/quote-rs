use alloy::contract::Error as ContractError;
use alloy::primitives::{Address, Bytes, FixedBytes, U256};
use alloy::sol_types::{SolError, SolValue};

use crate::abi::uniswap_v4_quote_single::UniswapV4QuoteSingle::{
    AmountOut, InsufficientLiquidity, PoolNotInitialized,
};
use crate::abi::uniswap_v4_quote_single::{
    PoolKey, QuoterConstructorArgs, UniswapV4QuoteSingle, UniswapV4QuoteSingleEphemeralByPoolId,
};
use crate::provider::MyProvider;
use crate::utils::revert_data_from_error;

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

/// Quote by poolId. Fetches poolManager and PoolKey from PositionManager on-chain.
/// `pool_id` must be 32 bytes (bytes32 = keccak256(abi.encode(poolKey))); contract strips to bytes25 for lookup.
pub async fn quote_by_pool_id(
    position_manager: Address,
    pool_id: FixedBytes<32>,
    token_in: Address,
    amount_in: u128,
    protocol_fee_bps: u128,
    provider: MyProvider,
) -> Result<u128, Box<dyn std::error::Error>> {
    let amount_in_u256 = U256::from(amount_in);

    let result: Result<Bytes, ContractError> =
        UniswapV4QuoteSingleEphemeralByPoolId::deploy_builder(
            provider.clone(),
            position_manager,
            pool_id,
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
