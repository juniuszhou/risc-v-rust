//! Uniswap V2 Pair - AMM with ERC20 LP token
#![no_main]
#![no_std]

use alloy_core::{sol, sol_types::SolCall};
use alloy_primitives::U256;
use pallet_revive_uapi::{CallFlags, HostFn, HostFnImpl as api, ReturnFlags};
use uniswap_rust::{mapping_key_1, mapping_key_2, math, storage_get_u256, storage_set_u256};

extern crate alloc;
use alloc::vec;

sol!("IUniswapV2Pair.sol");

const MINIMUM_LIQUIDITY: U256 = U256::from_limbs([1000, 0, 0, 0]); // 10^3

// Storage slots
const SLOT_FACTORY: u32 = 0;
const SLOT_TOKEN0: u32 = 1;
const SLOT_TOKEN1: u32 = 2;
const SLOT_RESERVES: u32 = 3;
const SLOT_PRICE0_CUMULATIVE: u32 = 4;
const SLOT_PRICE1_CUMULATIVE: u32 = 5;
const SLOT_K_LAST: u32 = 6;
const SLOT_UNLOCKED: u32 = 7;
const SLOT_TOTAL_SUPPLY: u32 = 8;
const SLOT_BALANCE_OF: u32 = 9;
const SLOT_ALLOWANCE: u32 = 10;

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

fn return_u256(v: U256) -> ! {
    let buf = v.to_be_bytes::<32>();
    api::return_value(ReturnFlags::empty(), buf.as_slice());
}

fn return_bool(v: bool) -> ! {
    return_u256(if v { U256::from(1) } else { U256::ZERO });
}

fn return_tuple_u112_u112_u32(r0: u128, r1: u128, ts: u32) -> ! {
    let mut buf = [0u8; 96];
    buf[0..32].copy_from_slice(&U256::from(r0).to_be_bytes::<32>());
    buf[32..64].copy_from_slice(&U256::from(r1).to_be_bytes::<32>());
    buf[64..96].copy_from_slice(&U256::from(ts).to_be_bytes::<32>());
    api::return_value(ReturnFlags::empty(), &buf);
}

fn return_tuple_u256_u256(a0: U256, a1: U256) -> ! {
    let mut buf = [0u8; 64];
    buf[0..32].copy_from_slice(&a0.to_be_bytes::<32>());
    buf[32..64].copy_from_slice(&a1.to_be_bytes::<32>());
    api::return_value(ReturnFlags::empty(), &buf);
}

fn get_factory() -> [u8; 20] {
    let v = storage_get_u256(SLOT_FACTORY);
    let mut out = [0u8; 20];
    out.copy_from_slice(&v.to_be_bytes::<32>()[12..32]);
    out
}

fn get_token0() -> [u8; 20] {
    let v = storage_get_u256(SLOT_TOKEN0);
    let mut out = [0u8; 20];
    out.copy_from_slice(&v.to_be_bytes::<32>()[12..32]);
    out
}

fn get_token1() -> [u8; 20] {
    let v = storage_get_u256(SLOT_TOKEN1);
    let mut out = [0u8; 20];
    out.copy_from_slice(&v.to_be_bytes::<32>()[12..32]);
    out
}

fn get_reserves() -> (u128, u128, u32) {
    let packed = storage_get_u256(SLOT_RESERVES);
    let r0: U256 = (packed >> 144) & U256::from(u128::MAX);
    let r1: U256 = (packed >> 32) & U256::from(u128::MAX);
    let ts: u32 = (packed & U256::from(u32::MAX)).to::<u32>();
    (r0.to::<u128>(), r1.to::<u128>(), ts)
}

fn set_reserves(r0: u128, r1: u128, ts: u32) {
    let packed = U256::from(r0) << 144 | U256::from(r1) << 32 | U256::from(ts);
    storage_set_u256(SLOT_RESERVES, packed);
}

fn get_balance(owner: &[u8; 20]) -> U256 {
    let key = mapping_key_1(SLOT_BALANCE_OF, owner);
    let mut out = [0u8; 32];
    api::get_storage_or_zero(pallet_revive_uapi::StorageFlags::empty(), &key, &mut out);
    U256::from_be_slice(&out)
}

fn set_balance(owner: &[u8; 20], value: U256) {
    let key = mapping_key_1(SLOT_BALANCE_OF, owner);
    let vb = value.to_be_bytes::<32>();
    api::set_storage_or_clear(pallet_revive_uapi::StorageFlags::empty(), &key, &vb);
}

fn get_allowance(owner: &[u8; 20], spender: &[u8; 20]) -> U256 {
    let key = mapping_key_2(SLOT_ALLOWANCE, owner, spender);
    let mut out = [0u8; 32];
    api::get_storage_or_zero(pallet_revive_uapi::StorageFlags::empty(), &key, &mut out);
    U256::from_be_slice(&out)
}

fn set_allowance(owner: &[u8; 20], spender: &[u8; 20], value: U256) {
    let key = mapping_key_2(SLOT_ALLOWANCE, owner, spender);
    let vb = value.to_be_bytes::<32>();
    api::set_storage_or_clear(pallet_revive_uapi::StorageFlags::empty(), &key, &vb);
}

fn safe_transfer(token: &[u8; 20], to: &[u8; 20], value: U256) {
    let mut data = vec![0u8; 4 + 32 + 32];
    data[0..4].copy_from_slice(&alloy_primitives::keccak256(b"transfer(address,uint256)")[..4]);
    data[16..36].copy_from_slice(to);
    data[36..68].copy_from_slice(&value.to_be_bytes::<32>());
    let zero = [0u8; 32];
    let res = api::call_evm(CallFlags::empty(), token, u64::MAX, &zero, &data, None);
    if res.is_err() {
        revert();
    }
}

fn safe_balance_of(token: &[u8; 20], account: &[u8; 20]) -> U256 {
    let mut data = vec![0u8; 4 + 32];
    data[0..4].copy_from_slice(&alloy_primitives::keccak256(b"balanceOf(address)")[..4]);
    data[16..36].copy_from_slice(account);
    let zero = [0u8; 32];
    let mut output = vec![0u8; 32];
    let mut output_slice: &mut [u8] = output.as_mut_slice();
    let res = api::call_evm(
        CallFlags::empty(),
        token,
        u64::MAX,
        &zero,
        &data,
        Some(&mut output_slice),
    );
    if res.is_err() {
        revert();
    }
    U256::from_be_slice(&output)
}

fn require_unlocked() {
    if storage_get_u256(SLOT_UNLOCKED) != U256::from(1) {
        revert();
    }
    storage_set_u256(SLOT_UNLOCKED, U256::ZERO);
}

fn set_unlocked() {
    storage_set_u256(SLOT_UNLOCKED, U256::from(1));
}

fn now_secs() -> u32 {
    let mut buf = [0u8; 32];
    api::now(&mut buf);
    U256::from_be_slice(&buf).to::<u32>()
}

#[polkavm_derive::polkavm_export]
pub extern "C" fn deploy() {
    let mut caller = [0u8; 20];
    api::caller(&mut caller);
    storage_set_u256(
        SLOT_FACTORY,
        U256::from_be_slice(&{
            let mut b = [0u8; 32];
            b[12..32].copy_from_slice(&caller);
            b
        }),
    );
    storage_set_u256(SLOT_UNLOCKED, U256::from(1));
}

#[polkavm_derive::polkavm_export]
pub extern "C" fn call() {
    let call_data_len = api::call_data_size();
    let mut call_data = vec![0u8; call_data_len as usize];
    api::call_data_copy(&mut call_data, 0);

    if call_data.len() < 4 {
        revert();
    }
    let selector: [u8; 4] = call_data[0..4].try_into().unwrap();

    // Pair view functions
    if selector == IUniswapV2Pair::factoryCall::SELECTOR {
        return_u256(U256::from_be_slice(&{
            let mut b = [0u8; 32];
            b[12..32].copy_from_slice(&get_factory());
            b
        }));
    }
    if selector == IUniswapV2Pair::token0Call::SELECTOR {
        return_u256(U256::from_be_slice(&{
            let mut b = [0u8; 32];
            b[12..32].copy_from_slice(&get_token0());
            b
        }));
    }
    if selector == IUniswapV2Pair::token1Call::SELECTOR {
        return_u256(U256::from_be_slice(&{
            let mut b = [0u8; 32];
            b[12..32].copy_from_slice(&get_token1());
            b
        }));
    }
    if selector == IUniswapV2Pair::getReservesCall::SELECTOR {
        let (r0, r1, ts) = get_reserves();
        return_tuple_u112_u112_u32(r0, r1, ts);
    }
    if selector == IUniswapV2Pair::totalSupplyCall::SELECTOR {
        return_u256(storage_get_u256(SLOT_TOTAL_SUPPLY));
    }
    if selector == IUniswapV2Pair::balanceOfCall::SELECTOR {
        let decoded =
            IUniswapV2Pair::balanceOfCall::abi_decode(&call_data[..]).unwrap_or_else(|_| revert());
        return_u256(get_balance(decoded.owner.as_slice().try_into().unwrap()));
    }
    if selector == IUniswapV2Pair::allowanceCall::SELECTOR {
        let decoded =
            IUniswapV2Pair::allowanceCall::abi_decode(&call_data[..]).unwrap_or_else(|_| revert());
        return_u256(get_allowance(
            decoded.owner.as_slice().try_into().unwrap(),
            decoded.spender.as_slice().try_into().unwrap(),
        ));
    }
    if selector == IUniswapV2Pair::MINIMUM_LIQUIDITYCall::SELECTOR {
        return_u256(MINIMUM_LIQUIDITY);
    }
    if selector == IUniswapV2Pair::price0CumulativeLastCall::SELECTOR {
        return_u256(storage_get_u256(SLOT_PRICE0_CUMULATIVE));
    }
    if selector == IUniswapV2Pair::price1CumulativeLastCall::SELECTOR {
        return_u256(storage_get_u256(SLOT_PRICE1_CUMULATIVE));
    }
    if selector == IUniswapV2Pair::kLastCall::SELECTOR {
        return_u256(storage_get_u256(SLOT_K_LAST));
    }

    // Mutating functions - lock
    if selector == IUniswapV2Pair::initializeCall::SELECTOR {
        require_unlocked();
        let decoded =
            IUniswapV2Pair::initializeCall::abi_decode(&call_data[..]).unwrap_or_else(|_| revert());
        if storage_get_u256(SLOT_TOKEN0) != U256::ZERO {
            set_unlocked();
            revert();
        }
        storage_set_u256(
            SLOT_TOKEN0,
            U256::from_be_slice(&{
                let mut b = [0u8; 32];
                b[12..32].copy_from_slice(decoded.token0_.as_slice());
                b
            }),
        );
        storage_set_u256(
            SLOT_TOKEN1,
            U256::from_be_slice(&{
                let mut b = [0u8; 32];
                b[12..32].copy_from_slice(decoded.token1_.as_slice());
                b
            }),
        );
        set_unlocked();
        api::return_value(ReturnFlags::empty(), &[]);
    }

    if selector == IUniswapV2Pair::mintCall::SELECTOR {
        require_unlocked();
        let decoded =
            IUniswapV2Pair::mintCall::abi_decode(&call_data[..]).unwrap_or_else(|_| revert());
        let to = decoded.to.0;
        let (reserve0, reserve1, _) = get_reserves();
        let token0 = get_token0();
        let token1 = get_token1();
        let balance0 = safe_balance_of(&token0, &{
            let mut me = [0u8; 20];
            api::address(&mut me);
            me
        });
        let balance1 = safe_balance_of(&token1, &{
            let mut me = [0u8; 20];
            api::address(&mut me);
            me
        });
        let amount0 = balance0 - U256::from(reserve0);
        let amount1 = balance1 - U256::from(reserve1);
        let total_supply = storage_get_u256(SLOT_TOTAL_SUPPLY);
        let liquidity = if total_supply == U256::ZERO {
            let liq = math::sqrt_u256(amount0 * amount1);
            if liq <= MINIMUM_LIQUIDITY {
                set_unlocked();
                revert();
            }
            let liq = liq - MINIMUM_LIQUIDITY;
            storage_set_u256(SLOT_TOTAL_SUPPLY, total_supply + MINIMUM_LIQUIDITY);
            set_balance(&[0u8; 20], MINIMUM_LIQUIDITY);
            liq
        } else {
            let r0 = U256::from(reserve0);
            let r1 = U256::from(reserve1);
            let l0 = amount0 * total_supply / r0;
            let l1 = amount1 * total_supply / r1;
            let liq = if l0 < l1 { l0 } else { l1 };
            if liq == U256::ZERO {
                set_unlocked();
                revert();
            }
            storage_set_u256(SLOT_TOTAL_SUPPLY, total_supply + liq);
            liq
        };
        set_balance(&to, get_balance(&to) + liquidity);
        set_reserves(balance0.to(), balance1.to(), now_secs());
        set_unlocked();
        return_u256(liquidity);
    }

    if selector == IUniswapV2Pair::burnCall::SELECTOR {
        require_unlocked();
        let decoded =
            IUniswapV2Pair::burnCall::abi_decode(&call_data[..]).unwrap_or_else(|_| revert());
        let to = decoded.to.0;
        let mut self_addr = [0u8; 20];
        api::address(&mut self_addr);
        let liquidity = get_balance(&self_addr);
        if liquidity == U256::ZERO {
            set_unlocked();
            revert();
        }
        let total_supply = storage_get_u256(SLOT_TOTAL_SUPPLY);
        let token0 = get_token0();
        let token1 = get_token1();
        let balance0 = safe_balance_of(&token0, &self_addr);
        let balance1 = safe_balance_of(&token1, &self_addr);
        let amount0 = liquidity * balance0 / total_supply;
        let amount1 = liquidity * balance1 / total_supply;
        if amount0 == U256::ZERO || amount1 == U256::ZERO {
            set_unlocked();
            revert();
        }
        set_balance(&self_addr, U256::ZERO);
        storage_set_u256(SLOT_TOTAL_SUPPLY, total_supply - liquidity);
        safe_transfer(&token0, &to, amount0);
        safe_transfer(&token1, &to, amount1);
        let new_bal0 = safe_balance_of(&token0, &self_addr);
        let new_bal1 = safe_balance_of(&token1, &self_addr);
        set_reserves(new_bal0.to(), new_bal1.to(), now_secs());
        set_unlocked();
        return_tuple_u256_u256(amount0, amount1);
    }

    if selector == IUniswapV2Pair::swapCall::SELECTOR {
        require_unlocked();
        let decoded =
            IUniswapV2Pair::swapCall::abi_decode(&call_data[..]).unwrap_or_else(|_| revert());
        let amount0_out = decoded.amount0Out;
        let amount1_out = decoded.amount1Out;
        let to = decoded.to.0;
        if amount0_out == U256::ZERO && amount1_out == U256::ZERO {
            set_unlocked();
            revert();
        }
        let (reserve0, reserve1, _) = get_reserves();
        if amount0_out >= U256::from(reserve0) || amount1_out >= U256::from(reserve1) {
            set_unlocked();
            revert();
        }
        let token0 = get_token0();
        let token1 = get_token1();
        if to == token0 || to == token1 {
            set_unlocked();
            revert();
        }
        if amount0_out > U256::ZERO {
            safe_transfer(&token0, &to, amount0_out);
        }
        if amount1_out > U256::ZERO {
            safe_transfer(&token1, &to, amount1_out);
        }
        if decoded.data.len() > 0 {
            // Skip flash swap callback for simplicity
        }
        let balance0 = safe_balance_of(&token0, &{
            let mut me = [0u8; 20];
            api::address(&mut me);
            me
        });
        let balance1 = safe_balance_of(&token1, &{
            let mut me = [0u8; 20];
            api::address(&mut me);
            me
        });
        let amount0_in = if balance0 > U256::from(reserve0) - amount0_out {
            balance0 - (U256::from(reserve0) - amount0_out)
        } else {
            U256::ZERO
        };
        let amount1_in = if balance1 > U256::from(reserve1) - amount1_out {
            balance1 - (U256::from(reserve1) - amount1_out)
        } else {
            U256::ZERO
        };
        if amount0_in == U256::ZERO && amount1_in == U256::ZERO {
            set_unlocked();
            revert();
        }
        let adj0 = balance0 * U256::from(1000) - amount0_in * U256::from(3);
        let adj1 = balance1 * U256::from(1000) - amount1_in * U256::from(3);
        if adj0 * adj1
            < U256::from(reserve0) * U256::from(reserve1) * U256::from(1000) * U256::from(1000)
        {
            set_unlocked();
            revert();
        }
        set_reserves(balance0.to(), balance1.to(), now_secs());
        set_unlocked();
        api::return_value(ReturnFlags::empty(), &[]);
    }

    if selector == IUniswapV2Pair::skimCall::SELECTOR {
        require_unlocked();
        let decoded =
            IUniswapV2Pair::skimCall::abi_decode(&call_data[..]).unwrap_or_else(|_| revert());
        let to = decoded.to.0;
        let (reserve0, reserve1, _) = get_reserves();
        let token0 = get_token0();
        let token1 = get_token1();
        let balance0 = safe_balance_of(&token0, &{
            let mut me = [0u8; 20];
            api::address(&mut me);
            me
        });
        let balance1 = safe_balance_of(&token1, &{
            let mut me = [0u8; 20];
            api::address(&mut me);
            me
        });
        safe_transfer(&token0, &to, balance0 - U256::from(reserve0));
        safe_transfer(&token1, &to, balance1 - U256::from(reserve1));
        set_unlocked();
        api::return_value(ReturnFlags::empty(), &[]);
    }

    if selector == IUniswapV2Pair::syncCall::SELECTOR {
        require_unlocked();
        let token0 = get_token0();
        let token1 = get_token1();
        let balance0 = safe_balance_of(&token0, &{
            let mut me = [0u8; 20];
            api::address(&mut me);
            me
        });
        let balance1 = safe_balance_of(&token1, &{
            let mut me = [0u8; 20];
            api::address(&mut me);
            me
        });
        let (_, _, ts) = get_reserves();
        set_reserves(balance0.to(), balance1.to(), ts);
        set_unlocked();
        api::return_value(ReturnFlags::empty(), &[]);
    }

    // ERC20 functions
    if selector == IUniswapV2Pair::approveCall::SELECTOR {
        let decoded =
            IUniswapV2Pair::approveCall::abi_decode(&call_data[..]).unwrap_or_else(|_| revert());
        let mut sender = [0u8; 20];
        api::caller(&mut sender);
        set_allowance(&sender, &decoded.spender.0, decoded.value);
        return_bool(true);
    }
    if selector == IUniswapV2Pair::transferCall::SELECTOR {
        let decoded =
            IUniswapV2Pair::transferCall::abi_decode(&call_data[..]).unwrap_or_else(|_| revert());
        let mut sender = [0u8; 20];
        api::caller(&mut sender);
        let bal = get_balance(&sender);
        if bal < decoded.value {
            revert();
        }
        set_balance(&sender, bal - decoded.value);
        set_balance(&decoded.to.0, get_balance(&decoded.to.0) + decoded.value);
        return_bool(true);
    }
    if selector == IUniswapV2Pair::transferFromCall::SELECTOR {
        let decoded = IUniswapV2Pair::transferFromCall::abi_decode(&call_data[..])
            .unwrap_or_else(|_| revert());
        let mut sender = [0u8; 20];
        api::caller(&mut sender);
        let allow = get_allowance(&decoded.from.0, &sender);
        if allow != U256::MAX && allow < decoded.value {
            revert();
        }
        let bal = get_balance(&decoded.from.0);
        if bal < decoded.value {
            revert();
        }
        if allow != U256::MAX {
            set_allowance(&decoded.from.0, &sender, allow - decoded.value);
        }
        set_balance(&decoded.from.0, bal - decoded.value);
        set_balance(&decoded.to.0, get_balance(&decoded.to.0) + decoded.value);
        return_bool(true);
    }

    revert();
}
