//! EVM Runner - revm executor wrapper for benchmarking

use anyhow::{anyhow, Result};
use revm::{
    primitives::{
        AccountInfo, Address, Bytecode, Bytes, ExecutionResult, Output, TransactTo, TxEnv, U256,
    },
    Evm, InMemoryDB,
};

/// Contract address used for deployment
pub const CONTRACT_ADDRESS: Address = Address::new([0x42; 20]);

/// Caller address used for transactions
pub const CALLER_ADDRESS: Address = Address::new([0x01; 20]);

/// Gas limit for transactions (300M to support large iteration counts)
pub const GAS_LIMIT: u64 = 300_000_000;

/// EVM executor wrapper for benchmarking
pub struct RevmExecutor {
    db: InMemoryDB,
}

impl RevmExecutor {
    /// Create a new executor with empty state
    pub fn new() -> Self {
        let mut db = InMemoryDB::default();

        // Set up caller account with balance
        db.insert_account_info(
            CALLER_ADDRESS,
            AccountInfo {
                balance: U256::from(1_000_000_000_000_000_000u128), // 1 ETH
                nonce: 0,
                code_hash: Default::default(),
                code: None,
            },
        );

        Self { db }
    }

    /// Deploy bytecode to the contract address
    pub fn deploy(&mut self, bytecode: impl Into<Bytes>) -> Result<()> {
        let bytecode = bytecode.into();
        let code = Bytecode::new_raw(bytecode);

        self.db.insert_account_info(
            CONTRACT_ADDRESS,
            AccountInfo {
                balance: U256::ZERO,
                nonce: 1,
                code_hash: code.hash_slow(),
                code: Some(code),
            },
        );

        Ok(())
    }

    /// Execute a contract call with given calldata
    pub fn call(&mut self, calldata: impl Into<Bytes>) -> Result<(u64, Bytes)> {
        let calldata = calldata.into();
        let tx = TxEnv {
            caller: CALLER_ADDRESS,
            gas_limit: GAS_LIMIT,
            gas_price: U256::from(1),
            transact_to: TransactTo::Call(CONTRACT_ADDRESS),
            value: U256::ZERO,
            data: calldata,
            nonce: None,
            chain_id: Some(1),
            access_list: vec![],
            gas_priority_fee: None,
            blob_hashes: vec![],
            max_fee_per_blob_gas: None,
            authorization_list: None,
        };

        let mut evm = Evm::builder().with_db(&mut self.db).with_tx_env(tx).build();

        let result = evm.transact_commit()?;

        match result {
            ExecutionResult::Success {
                gas_used, output, ..
            } => {
                let data = match output {
                    Output::Call(data) => data,
                    Output::Create(data, _) => data,
                };
                Ok((gas_used, data))
            }
            ExecutionResult::Revert { gas_used, output } => Err(anyhow!(
                "Execution reverted: gas_used={}, output={:?}",
                gas_used,
                output
            )),
            ExecutionResult::Halt { reason, gas_used } => Err(anyhow!(
                "Execution halted: {:?}, gas_used={}",
                reason,
                gas_used
            )),
        }
    }

    /// Reset storage state (useful between benchmark iterations)
    pub fn reset_storage(&mut self) {
        // Clear storage for contract address
        if let Some(account) = self.db.accounts.get_mut(&CONTRACT_ADDRESS) {
            account.storage.clear();
        }
    }
}

impl Default for RevmExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Benchmark arithmetic operations on revm
fn bench_revm_arithmetic() {
    // Load bytecode
    let bytecode = load_evm_bytecode() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Skipping revm arithmetic benchmark: {}", e);
            return;
        }
    };

    
        let mut executor = RevmExecutor::new();
        executor.deploy(bytecode.clone()).unwrap();
        let calldata = contracts::arithmetic::encode_compute(U256::from(12345), U256::from(6789));

        let result = executor.call(calldata.clone()).unwrap();
    
}
