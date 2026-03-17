use alloy::contract::Error as ContractError;
use alloy::primitives::{Address, Bytes, U256};
use alloy::sol;
use alloy::sol_types::{SolError, SolValue};

use crate::abi::uniswap_v2_quote_single::UniswapV2QuoteSingle::{self, AmountOut};
use crate::provider::MyProvider;

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

/// Extracts revert data from contract error. Alloy's `as_revert_data()` only returns data when
/// `message.contains("revert")`; some RPCs (e.g. Fuse) return "VM execution error." so we also
/// parse the error payload's `data` field when it starts with "Reverted 0x".
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
