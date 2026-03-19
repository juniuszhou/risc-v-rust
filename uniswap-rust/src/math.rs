//! Pure math utilities (no host deps - testable)

use alloy_primitives::U256;

/// Babylonian sqrt for U256 - matches Uniswap V2 Math library
pub fn sqrt_u256(y: U256) -> U256 {
    if y > U256::from(3) {
        let mut z = y;
        let mut x = y / U256::from(2) + U256::from(1);
        while x < z {
            z = x;
            x = (y / x + x) / U256::from(2);
        }
        z
    } else if y != U256::ZERO {
        U256::from(1)
    } else {
        U256::ZERO
    }
}

