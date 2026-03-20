use alloy::contract::Error as ContractError;
use alloy::primitives::{Address, Bytes, U256};
use alloy::sol;
use alloy::sol_types::{SolError, SolValue};

use crate::abi::uniswap_v2_quote_single::UniswapV2QuoteSingle::{self, AmountOut};
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
    let bytecode = UniswapV2QuoteSingle::BYTECODE.as_ref().to_vec();
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
    let amount_in_u256 = alloy::primitives::U256::from(amount_in);

    let result: Result<Bytes, ContractError> = UniswapV2QuoteSingle::deploy_builder(
        provider.clone(),
        pool_id,
        token_in,
        amount_in_u256,
        U256::from(protocol_fee_bps),
    )
    .call()
    .await;

    let amount_out = match &result {
        Ok(_) => return Err("Ephemeral quoter returned success (unexpected)".into()),
        Err(e) => {
            if let Some(decoded) = e.as_decoded_error::<AmountOut>() {
                decoded.amountOut.to::<u128>()
            } else if let Some(bytes) = revert_data_from_error(e) {
                let decoded = AmountOut::abi_decode(bytes.as_ref())
                    .map_err(|_| "Could not decode AmountOut from revert")?;
                decoded.amountOut.to::<u128>()
            } else {
                return Err(format!("Could not decode revert: {e:?}").into());
            }
        }
    };

    Ok(amount_out)
}
