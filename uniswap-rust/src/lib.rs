#![no_std]

pub mod math;

#[cfg(target_arch = "riscv64")]
mod storage {
    use alloy_primitives::U256;
    use pallet_revive_uapi::{HostFn, HostFnImpl as api, StorageFlags};

    #[inline(always)]
    pub fn mapping_key_1(base_slot: u32, key: &[u8; 20]) -> [u8; 32] {
        let mut input = [0u8; 64];
        input[12..32].copy_from_slice(key);
        input[44..48].copy_from_slice(&base_slot.to_be_bytes());
        let mut out = [0u8; 32];
        api::hash_keccak_256(&input, &mut out);
        out
    }

    #[inline(always)]
    pub fn mapping_key_2(base_slot: u32, key1: &[u8; 20], key2: &[u8; 20]) -> [u8; 32] {
        let mut inner = [0u8; 64];
        inner[12..32].copy_from_slice(key2);
        inner[44..48].copy_from_slice(&(base_slot + 1).to_be_bytes());
        let mut inner_hash = [0u8; 32];
        api::hash_keccak_256(&inner, &mut inner_hash);
        let mut outer = [0u8; 64];
        outer[12..32].copy_from_slice(key1);
        outer[32..64].copy_from_slice(&inner_hash);
        let mut out = [0u8; 32];
        api::hash_keccak_256(&outer, &mut out);
        out
    }

    #[inline(always)]
    pub fn storage_get_u256(slot: u32) -> U256 {
        let key = slot.to_be_bytes();
        let mut key_arr = [0u8; 32];
        key_arr[28..32].copy_from_slice(&key);
        let mut out = [0u8; 32];
        api::get_storage_or_zero(StorageFlags::empty(), &key_arr, &mut out);
        U256::from_be_slice(&out)
    }

    #[inline(always)]
    pub fn storage_set_u256(slot: u32, value: U256) {
        let mut key = [0u8; 32];
        key[28..32].copy_from_slice(&slot.to_be_bytes());
        let value_bytes = value.to_be_bytes::<32>();
        api::set_storage_or_clear(StorageFlags::empty(), &key, &value_bytes);
    }
}

#[cfg(target_arch = "riscv64")]
pub use storage::*;
