//! Balance slot finding logic ported from simulator (SloadInspector + mutation testing).
//! Works with any CacheDB<DB> where DB implements DatabaseRef.

use alloy::primitives::{Address, U256};
use alloy::sol;
use alloy::sol_types::{SolCall, SolValue};
use revm::context::{TxEnv, tx::TxEnvBuildError};
use revm::context_interface::result::ExecutionResult;
use revm::database::{CacheDB, DatabaseRef, EmptyDB, WrapDatabaseRef};
use revm::interpreter::interpreter::EthInterpreter;
use revm::interpreter::interpreter_types::Jumps;
use revm::interpreter::{CallInputs, CallOutcome, Interpreter};
use revm::primitives::{HashSet, TxKind};
use revm::{Context, ExecuteEvm, InspectEvm, Inspector, MainBuilder, MainContext};
use thiserror::Error;

sol! {
    interface IERC20 {
        function balanceOf(address account) external view returns (uint256);
        function approve(address spender, uint256 value) external returns (bool);
    }
}

const SLOAD_OPCODE: u8 = 0x54;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SlotWithAddress {
    pub address: Address,
    pub slot: U256,
}

#[derive(Default)]
struct SloadInspector {
    slots: HashSet<SlotWithAddress>,
    current_address: Address,
}

impl<CTX> Inspector<CTX, EthInterpreter> for SloadInspector {
    fn step(&mut self, interp: &mut Interpreter<EthInterpreter>, _: &mut CTX) {
        if interp.bytecode.opcode() != SLOAD_OPCODE {
            return;
        }
        if let Ok(slot) = interp.stack.peek(0) {
            self.slots.insert(SlotWithAddress {
                address: self.current_address,
                slot,
            });
        }
    }

    fn call(&mut self, _: &mut CTX, inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.current_address = inputs.target_address;
        None
    }
}

#[derive(Debug, Error)]
#[error("finding balance slot failed")]
pub enum FindSlotError {
    #[error("inspecting balanceOf: {0}")]
    Inspect(#[from] InspectBalanceOfError),
    #[error("slot by mutation: {0}")]
    Mutation(#[from] FindSlotByMutationError),
}

#[derive(Debug, Error)]
#[error("inspecting balanceOf call failed")]
pub enum InspectBalanceOfError {
    #[error("tx build: {0}")]
    TxBuild(#[from] TxEnvBuildError),
    #[error("evm: {0}")]
    Evm(String),
    #[error("execution failed")]
    Execution,
}

#[derive(Debug, Error)]
#[error("finding slot by mutation failed")]
pub struct FindSlotByMutationError;

fn build_balance_of_tx_env(
    token_address: Address,
    user_address: Address,
) -> Result<TxEnv, TxEnvBuildError> {
    let encoded = IERC20::balanceOfCall {
        account: user_address,
    }
    .abi_encode();
    TxEnv::builder()
        .kind(TxKind::Call(token_address))
        .data(encoded.into())
        .build()
}

/// Finds the storage slot for an ERC20 balance by inspecting SLOAD during balanceOf
/// and verifying via mutation. Works with any fork DB (e.g. SharedBackend).
pub fn find_balance_slot<DB>(
    token_address: Address,
    user_address: Address,
    cache_db: &mut CacheDB<WrapDatabaseRef<DB>>,
) -> Result<SlotWithAddress, FindSlotError>
where
    DB: DatabaseRef,
{
    let inspector = inspect_balance_of(token_address, user_address, cache_db)?;
    let cached_accounts = cache_db.cache.accounts.clone();
    let mut isolated_db = CacheDB::new(EmptyDB::default());
    isolated_db.cache.accounts = cached_accounts;
    let slot = find_slot_by_mutation(user_address, token_address, &inspector, &mut isolated_db)?;
    Ok(slot)
}

fn inspect_balance_of<DB>(
    token_address: Address,
    user_address: Address,
    cache_db: &mut CacheDB<WrapDatabaseRef<DB>>,
) -> Result<SloadInspector, InspectBalanceOfError>
where
    DB: DatabaseRef,
{
    let inspector = SloadInspector::default();
    let mut evm = Context::mainnet()
        .with_db(cache_db)
        .modify_cfg_chained(|cfg| cfg.disable_nonce_check = true)
        .build_mainnet_with_inspector(inspector);
    let tx = build_balance_of_tx_env(token_address, user_address)?;
    let res = evm
        .inspect_one_tx(tx)
        .map_err(|e| InspectBalanceOfError::Evm(e.to_string()))?;
    match res {
        ExecutionResult::Success {
            reason: revm::context::result::SuccessReason::Return,
            ..
        } => Ok(evm.inspector),
        _ => Err(InspectBalanceOfError::Execution),
    }
}

fn balance_of(
    user_address: Address,
    token_address: Address,
    cache_db: &mut CacheDB<EmptyDB>,
) -> Result<U256, InspectBalanceOfError> {
    let mut evm = Context::mainnet()
        .with_db(cache_db)
        .modify_cfg_chained(|cfg| cfg.disable_nonce_check = true)
        .build_mainnet();
    let tx = build_balance_of_tx_env(token_address, user_address)?;
    let res = evm
        .transact_one(tx)
        .map_err(|e| InspectBalanceOfError::Evm(e.to_string()))?;
    match res {
        ExecutionResult::Success { output, .. } => {
            U256::abi_decode(output.data()).map_err(|_| InspectBalanceOfError::Execution)
        }
        _ => Err(InspectBalanceOfError::Execution),
    }
}

const TARGET_VALUE: U256 = U256::from_limbs([1234567890, 0, 0, 0]);

fn find_slot_by_mutation(
    user_address: Address,
    token_address: Address,
    inspector: &SloadInspector,
    cache_db: &mut CacheDB<EmptyDB>,
) -> Result<SlotWithAddress, FindSlotByMutationError> {
    for slot_with_address in &inspector.slots {
        if let Ok(new_balance) = test_slot(user_address, token_address, slot_with_address, cache_db)
        {
            if new_balance == TARGET_VALUE {
                return Ok(slot_with_address.clone());
            }
        }
    }
    Err(FindSlotByMutationError)
}

fn test_slot(
    user_address: Address,
    token_address: Address,
    slot_with_address: &SlotWithAddress,
    cache_db: &mut CacheDB<EmptyDB>,
) -> Result<U256, InspectBalanceOfError> {
    let acc = cache_db
        .load_account(slot_with_address.address)
        .expect("isolated_db has copied accounts");
    let original_value = acc.storage.get(&slot_with_address.slot).copied();
    acc.storage.insert(slot_with_address.slot, TARGET_VALUE);
    let new_balance = balance_of(user_address, token_address, cache_db);
    let acc = cache_db
        .load_account(slot_with_address.address)
        .expect("isolated_db has copied accounts");
    match original_value {
        Some(v) => {
            acc.storage.insert(slot_with_address.slot, v);
        }
        None => {
            acc.storage.remove(&slot_with_address.slot);
        }
    }
    new_balance
}
