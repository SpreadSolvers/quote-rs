use alloy::contract::Error as ContractError;
use alloy::primitives::{Address, Bytes, U256};
use alloy::sol;
use alloy::sol_types::{SolError, SolValue};

use crate::abi::uniswap_v3_quote_single::UniswapV3QuoteSingle::{
    self, AmountOut, InsufficientLiquidity, PartialFill,
};
use crate::provider::MyProvider;
use crate::utils::revert_data_from_error;

sol! {
    struct QuoterConstructorArgs {
        address pool;
        address tokenIn;
        uint256 amountIn;
        uint256 protocolFeeBps;
    }
}

/// Returns deployment calldata for the quoter (bytecode + encoded constructor args).
/// Used to run the quoter on a revm instance via CREATE tx.
pub fn quoter_deployment_data(
    pool_id: Address,
    token_in: Address,
    amount_in: u128,
    protocol_fee_bps: u128,
) -> Bytes {
    let bytecode = UniswapV3QuoteSingle::BYTECODE.as_ref().to_vec();
    let args = QuoterConstructorArgs {
        pool: pool_id,
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
    pool_id: Address,
    token_in: Address,
    amount_in: u128,
    protocol_fee_bps: u128,
    provider: MyProvider,
) -> Result<u128, Box<dyn std::error::Error>> {
    let amount_in_u256 = U256::from(amount_in);

    let result: Result<Bytes, ContractError> = UniswapV3QuoteSingle::deploy_builder(
        provider.clone(),
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
    if let Ok(pf) = PartialFill::abi_decode(bytes.as_ref()) {
        return Err(format!(
            "insufficient liquidity: only {} of {} input consumed, would get {} out",
            pf.amountInConsumed, amount_in, pf.amountOut
        )
        .into());
    }

    let decoded = AmountOut::abi_decode(bytes.as_ref())
        .map_err(|_| format!("Could not decode revert: {err:?}"))?;

    Ok(decoded.amountOut.to::<u128>())
}
