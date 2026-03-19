//! Uniswap V2 Factory - Creates pair contracts
#![no_main]
#![no_std]

use alloy_core::{sol, sol_types::SolCall};
use alloy_primitives::U256;
use pallet_revive_uapi::{CallFlags, HostFn, HostFnImpl as api, ReturnFlags, StorageFlags};
use uniswap_rust::{mapping_key_2, storage_get_u256, storage_set_u256};

extern crate alloc;
use alloc::vec;

sol!("IUniswapV2Factory.sol");

// Storage slots
const SLOT_FEE_TO: u32 = 0;
const SLOT_FEE_TO_SETTER: u32 = 1;
const SLOT_GET_PAIR: u32 = 2; // mapping(token0 => mapping(token1 => pair))
const SLOT_ALL_PAIRS: u32 = 3; // array - length at 3, data at keccak(3)+i
const SLOT_PAIR_CODE_HASH: u32 = 4;

#[global_allocator]
static mut ALLOC: picoalloc::Mutex<picoalloc::Allocator<picoalloc::ArrayPointer<4096>>> = {
    static mut ARRAY: picoalloc::Array<4096> = picoalloc::Array([0u8; 4096]);
    picoalloc::Mutex::new(picoalloc::Allocator::new(unsafe {
        picoalloc::ArrayPointer::new(&raw mut ARRAY)
    }))
};

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        core::arch::asm!("unimp");
        core::hint::unreachable_unchecked();
    }
}

fn revert() -> ! {
    api::return_value(ReturnFlags::REVERT, &[]);
}

fn return_address(addr: &[u8; 20]) -> ! {
    let mut buf = [0u8; 32];
    buf[12..32].copy_from_slice(addr);
    api::return_value(ReturnFlags::empty(), &buf);
}

fn return_u256(v: U256) -> ! {
    let buf = v.to_be_bytes::<32>();
    api::return_value(ReturnFlags::empty(), buf.as_slice());
}

fn get_pair_internal(token0: &[u8; 20], token1: &[u8; 20]) -> [u8; 20] {
    let key = mapping_key_2(SLOT_GET_PAIR, token0, token1);
    let mut out = [0u8; 32];
    api::get_storage_or_zero(StorageFlags::empty(), &key, &mut out);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&out[12..32]);
    addr
}

fn set_pair(token0: &[u8; 20], token1: &[u8; 20], pair: &[u8; 20]) {
    let key = mapping_key_2(SLOT_GET_PAIR, token0, token1);
    let mut value = [0u8; 32];
    value[12..32].copy_from_slice(pair);
    api::set_storage_or_clear(StorageFlags::empty(), &key, &value);
}

fn all_pairs_length() -> U256 {
    storage_get_u256(SLOT_ALL_PAIRS)
}

fn all_pairs_get(index: U256) -> [u8; 20] {
    let key = array_slot(SLOT_ALL_PAIRS, index);
    let mut out = [0u8; 32];
    api::get_storage_or_zero(StorageFlags::empty(), &key, &mut out);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&out[12..32]);
    addr
}

fn all_pairs_push(pair: &[u8; 20]) {
    let len = all_pairs_length();
    let key = array_slot(SLOT_ALL_PAIRS, len);
    let mut value = [0u8; 32];
    value[12..32].copy_from_slice(pair);
    api::set_storage_or_clear(StorageFlags::empty(), &key, &value);
    storage_set_u256(SLOT_ALL_PAIRS, len + U256::from(1));
}

// Pallet-revive doesn't have hash_keccak_256_slot - we need to compute array slot manually
fn array_slot(base: u32, index: U256) -> [u8; 32] {
    let mut packed = [0u8; 64];
    let base_bytes = {
        let mut b = [0u8; 32];
        b[28..32].copy_from_slice(&base.to_be_bytes());
        b
    };
    packed[0..32].copy_from_slice(&base_bytes);
    packed[32..64].copy_from_slice(&index.to_be_bytes::<32>());
    let mut out = [0u8; 32];
    api::hash_keccak_256(&packed, &mut out);
    out
}

#[polkavm_derive::polkavm_export]
pub extern "C" fn deploy() {
    // Constructor: feeToSetter (address) and pairCodeHash (bytes32) - 64 bytes ABI-encoded
    let call_data_len = api::call_data_size();
    if call_data_len < 64 {
        return;
    }
    let mut call_data = vec![0u8; call_data_len as usize];
    api::call_data_copy(&mut call_data[..], 0);
    let fee_to_setter = {
        let mut a = [0u8; 20];
        a.copy_from_slice(&call_data[12..32]); // address is left-padded to 32 bytes
        a
    };
    let pair_code_hash = {
        let mut h = [0u8; 32];
        h.copy_from_slice(&call_data[32..64]);
        h
    };
    storage_set_u256(
        SLOT_FEE_TO_SETTER,
        U256::from_be_slice(&{
            let mut b = [0u8; 32];
            b[12..32].copy_from_slice(&fee_to_setter);
            b
        }),
    );
    let mut key = [0u8; 32];
    key[28..32].copy_from_slice(&SLOT_PAIR_CODE_HASH.to_be_bytes());
    api::set_storage_or_clear(StorageFlags::empty(), &key, &pair_code_hash);
}

#[polkavm_derive::polkavm_export]
pub extern "C" fn call() {
    let call_data_len = api::call_data_size();
    let mut call_data = vec![0u8; call_data_len as usize];
    api::call_data_copy(&mut call_data[..], 0);

    if call_data.len() < 4 {
        revert();
    }
    let selector: [u8; 4] = call_data[0..4].try_into().unwrap();

    if selector == IUniswapV2Factory::feeToCall::SELECTOR {
        return_u256(storage_get_u256(SLOT_FEE_TO));
    }
    if selector == IUniswapV2Factory::feeToSetterCall::SELECTOR {
        return_u256(storage_get_u256(SLOT_FEE_TO_SETTER));
    }
    if selector == IUniswapV2Factory::getPairCall::SELECTOR {
        let decoded =
            IUniswapV2Factory::getPairCall::abi_decode(&call_data[..]).unwrap_or_else(|_| revert());
        let (token0, token1) = sort_tokens(
            decoded.tokenA.as_slice().try_into().unwrap(),
            decoded.tokenB.as_slice().try_into().unwrap(),
        );
        let pair = get_pair_internal(&token0, &token1);
        return_address(&pair);
    }
    if selector == IUniswapV2Factory::allPairsCall::SELECTOR {
        let decoded = IUniswapV2Factory::allPairsCall::abi_decode(&call_data[..])
            .unwrap_or_else(|_| revert());
        let pair = all_pairs_get(decoded.0);
        return_address(&pair);
    }
    if selector == IUniswapV2Factory::allPairsLengthCall::SELECTOR {
        return_u256(all_pairs_length());
    }

    if selector == IUniswapV2Factory::createPairCall::SELECTOR {
        let decoded = IUniswapV2Factory::createPairCall::abi_decode(&call_data[..])
            .unwrap_or_else(|_| revert());
        let token_a: [u8; 20] = decoded.tokenA.as_slice().try_into().unwrap();
        let token_b: [u8; 20] = decoded.tokenB.as_slice().try_into().unwrap();
        if token_a == token_b {
            revert();
        }
        let (token0, token1) = sort_tokens(&token_a, &token_b);
        if token0 == [0u8; 20] {
            revert();
        }
        if get_pair_internal(&token0, &token1) != [0u8; 20] {
            revert();
        }
        // Get pair code hash from storage
        let mut key = [0u8; 32];
        key[28..32].copy_from_slice(&SLOT_PAIR_CODE_HASH.to_be_bytes());
        let mut code_hash = [0u8; 32];
        api::get_storage_or_zero(StorageFlags::empty(), &key, &mut code_hash);
        if code_hash == [0u8; 32] {
            revert(); // Pair code not set
        }
        // Instantiate pair with empty constructor (pair deploy sets factory=caller)
        let mut input = vec![0u8; 32];
        input[0..32].copy_from_slice(&code_hash);
        let zero = [0u8; 32];
        let mut pair_addr = [0u8; 20];
        let res = api::instantiate(
            u64::MAX,
            u64::MAX,
            &zero,
            &zero,
            &input[..],
            Some(&mut pair_addr),
            None,
            None,
        );
        if res.is_err() {
            revert();
        }
        // Call pair.initialize(token0, token1)
        let init_data = encode_initialize(&token0, &token1);
        let call_res = api::call(
            CallFlags::empty(),
            &pair_addr,
            u64::MAX,
            u64::MAX,
            &zero,
            &zero,
            &init_data[..],
            None,
        );
        if call_res.is_err() {
            revert();
        }
        set_pair(&token0, &token1, &pair_addr);
        set_pair(&token1, &token0, &pair_addr);
        all_pairs_push(&pair_addr);
        return_address(&pair_addr);
    }

    if selector == IUniswapV2Factory::setFeeToCall::SELECTOR {
        let mut caller = [0u8; 20];
        api::caller(&mut caller);
        let setter = storage_get_u256(SLOT_FEE_TO_SETTER);
        let mut setter_addr = [0u8; 20];
        setter_addr.copy_from_slice(&setter.to_be_bytes::<32>()[12..32]);
        if caller != setter_addr {
            revert();
        }
        let decoded = IUniswapV2Factory::setFeeToCall::abi_decode(&call_data[..])
            .unwrap_or_else(|_| revert());
        let new_fee_to = decoded.0;
        storage_set_u256(
            SLOT_FEE_TO,
            U256::from_be_slice(&{
                let mut b = [0u8; 32];
                b[12..32].copy_from_slice(new_fee_to.as_slice());
                b
            }),
        );
        api::return_value(ReturnFlags::empty(), &[]);
    }

    if selector == IUniswapV2Factory::setFeeToSetterCall::SELECTOR {
        let mut caller = [0u8; 20];
        api::caller(&mut caller);
        let setter = storage_get_u256(SLOT_FEE_TO_SETTER);
        let mut setter_addr = [0u8; 20];
        setter_addr.copy_from_slice(&setter.to_be_bytes::<32>()[12..32]);
        if caller != setter_addr {
            revert();
        }
        let decoded = IUniswapV2Factory::setFeeToSetterCall::abi_decode(&call_data[..])
            .unwrap_or_else(|_| revert());
        let new_setter = decoded.0;
        storage_set_u256(
            SLOT_FEE_TO_SETTER,
            U256::from_be_slice(&{
                let mut b = [0u8; 32];
                b[12..32].copy_from_slice(new_setter.as_slice());
                b
            }),
        );
        api::return_value(ReturnFlags::empty(), &[]);
    }

    revert();
}

fn sort_tokens(a: &[u8; 20], b: &[u8; 20]) -> ([u8; 20], [u8; 20]) {
    if a < b {
        (*a, *b)
    } else {
        (*b, *a)
    }
}

fn encode_initialize(token0: &[u8; 20], token1: &[u8; 20]) -> alloc::vec::Vec<u8> {
    let selector = alloy_primitives::keccak256(b"initialize(address,address)");
    let mut data = vec![0u8; 4 + 32 + 32];
    data[0..4].copy_from_slice(&selector[..4]);
    data[16..36].copy_from_slice(token0);
    data[48..68].copy_from_slice(token1);
    data
}
