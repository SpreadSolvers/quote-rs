use alloy::contract::Error as ContractError;
use alloy::primitives::Bytes;

/// Extracts revert data from contract error.
pub fn revert_data_from_error(e: &ContractError) -> Option<Bytes> {
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
