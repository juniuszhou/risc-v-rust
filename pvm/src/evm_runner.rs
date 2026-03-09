//! EVM Runner - revm executor wrapper for benchmarking

use anyhow::{anyhow, Result};
use revm::{
    context::{result::ExecutionResult, transaction::AccessList, Context, TxEnv},
    context_interface::ContextTr,
    database::InMemoryDB,
    handler::{ExecuteCommitEvm, MainBuilder, MainContext, MainnetEvm},
    primitives::{Address, Bytes, TxKind, U256},
    state::{AccountInfo, Bytecode},
};

/// Contract address used for deployment
pub const CONTRACT_ADDRESS: Address = Address::new([0x42; 20]);

/// Caller address used for transactions
pub const CALLER_ADDRESS: Address = Address::new([0x01; 20]);

/// Gas limit for transactions (300M to support large iteration counts)
pub const GAS_LIMIT: u64 = 300_000_000;

/// EVM executor wrapper for benchmarking
pub struct RevmExecutor {
    evm: MainnetEvm<
        Context<
            revm::context::BlockEnv,
            TxEnv,
            revm::context::CfgEnv,
            InMemoryDB,
            revm::context::Journal<InMemoryDB>,
            (),
        >,
    >,
    /// Deployed contract address (set by deploy or deploy_create)
    contract_address: Address,
}

impl RevmExecutor {
    /// Create a new executor with empty state
    pub fn new() -> Self {
        let mut db = InMemoryDB::default();
        db.insert_account_info(
            CALLER_ADDRESS,
            AccountInfo {
                balance: U256::from(1_000_000_000_000_000_000u128), // 1 ETH
                nonce: 0,
                code_hash: Default::default(),
                account_id: Default::default(),
                code: None,
            },
        );

        let ctx = Context::mainnet()
            .modify_cfg_chained(|cfg| cfg.tx_gas_limit_cap = Some(GAS_LIMIT))
            .with_db(db);
        let evm = ctx.build_mainnet();
        Self {
            evm,
            contract_address: CONTRACT_ADDRESS,
        }
    }

    /// Deploy runtime bytecode directly to CONTRACT_ADDRESS (for hand-crafted or runtime-only code)
    pub fn deploy(&mut self, bytecode: impl Into<Bytes>) -> Result<()> {
        let bytecode = bytecode.into();
        let code = Bytecode::new_raw(bytecode);

        self.evm.db_mut().insert_account_info(
            CONTRACT_ADDRESS,
            AccountInfo {
                balance: U256::ZERO,
                nonce: 1,
                code_hash: code.hash_slow(),
                code: Some(code),
                account_id: Default::default(),
            },
        );
        self.contract_address = CONTRACT_ADDRESS;
        Ok(())
    }

    /// Deploy via CREATE - use for Solidity deployment bytecode (init + runtime).
    /// Returns the created contract address.
    pub fn deploy_create(&mut self, init_bytecode: impl Into<Bytes>) -> Result<Address> {
        let init_bytecode = init_bytecode.into();
        let tx = TxEnv {
            caller: CALLER_ADDRESS,
            kind: TxKind::Create,
            tx_type: 0,
            gas_limit: GAS_LIMIT,
            gas_price: 1u128,
            value: U256::ZERO,
            data: init_bytecode,
            nonce: 0,
            chain_id: Some(1),
            access_list: AccessList::default(),
            gas_priority_fee: None,
            blob_hashes: vec![],
            max_fee_per_blob_gas: 1_u128,
            authorization_list: vec![],
        };

        let result = self.evm.transact_commit(tx)?;
        match result {
            ExecutionResult::Success { output, .. } => {
                let addr = output
                    .address()
                    .copied()
                    .ok_or_else(|| anyhow!("CREATE did not return address"))?;
                self.contract_address = addr;
                Ok(addr)
            }
            ExecutionResult::Revert { output, .. } => Err(anyhow!("CREATE reverted: {:?}", output)),
            ExecutionResult::Halt { reason, .. } => Err(anyhow!("CREATE halted: {:?}", reason)),
        }
    }

    /// Execute a contract call with given calldata
    pub fn call(&mut self, calldata: impl Into<Bytes>) -> Result<(u64, Bytes)> {
        let calldata = calldata.into();
        let tx = TxEnv {
            caller: CALLER_ADDRESS,
            kind: TxKind::Call(self.contract_address),
            tx_type: 0,
            gas_limit: GAS_LIMIT,
            gas_price: 1u128,
            value: U256::ZERO,
            data: calldata,
            nonce: 0,
            chain_id: Some(1),
            access_list: AccessList::default(),
            gas_priority_fee: None,
            blob_hashes: vec![],
            max_fee_per_blob_gas: 1_u128,
            authorization_list: vec![],
        };

        let result = self.evm.transact_commit(tx)?;

        match result {
            ExecutionResult::Success { gas, output, .. } => Ok((gas.used(), output.into_data())),
            ExecutionResult::Revert { gas, output, .. } => Err(anyhow!(
                "Execution reverted: gas_used={}, output={:?}",
                gas.used(),
                output
            )),
            ExecutionResult::Halt { reason, gas, .. } => Err(anyhow!(
                "Execution halted: {:?}, gas_used={}",
                reason,
                gas.used()
            )),
        }
    }

    /// Reset storage state (useful between benchmark iterations)
    pub fn reset_storage(&mut self) {
        if let Some(account) = self
            .evm
            .db_mut()
            .cache
            .accounts
            .get_mut(&self.contract_address)
        {
            account.storage.clear();
        }
    }
}

impl Default for RevmExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evm_deploy_and_call() {
        let mut exec = RevmExecutor::new();
        // Deploy bytecode: PUSH1 0x42, PUSH1 0, MSTORE, PUSH1 1, PUSH1 31, RETURN (returns 0x42)
        let bytecode = Bytes::from([
            0x60, 0x42, // PUSH1 0x42
            0x60, 0x00, // PUSH1 0
            0x52, // MSTORE
            0x60, 0x01, // PUSH1 1
            0x60, 0x1f, // PUSH1 31
            0xf3, // RETURN
        ]);
        exec.deploy(bytecode).unwrap();
        let (gas_used, output) = exec.call(Bytes::new()).unwrap();
        assert!(gas_used > 0);
        assert_eq!(output.as_ref(), &[0x42]);
    }
}
