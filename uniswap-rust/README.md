# Uniswap V2 - Rust (Polkadot VM)

Uniswap V2 core contracts (Factory + Pair) implemented in Rust for the Polkadot VM (PolkaVM / pallet-revive) contract framework. Matches Uniswap V2 interface and ABI for EVM tooling compatibility.

## Prerequisites

- Rust nightly (`rustup default nightly`)
- [PolkaVM toolchain](https://docs.rs/cargo-pvm-contract-builder/)
- `polkatool` (for linking)

```bash
cargo install cargo-pvm-contract
# polkatool comes with the PolkaVM toolchain
```

## Build

```bash
./build.sh
```

Produces:
- `uniswap-v2-factory.polkavm` – Factory contract bytecode
- `uniswap-v2-pair.polkavm` – Pair contract bytecode

## Deployment (PolkaVM / pallet-revive)

1. **Upload Pair code** – Deploy the pair bytecode and record its code hash.
2. **Deploy Factory** – Call instantiate with:
   - Constructor args (ABI-encoded): `(feeToSetter: address, pairCodeHash: bytes32)`
3. **Create pairs** – Call `factory.createPair(tokenA, tokenB)`.

## Interface / ABI

- Factory: `abis/uniswap-v2-factory.json`
- Pair: see `IUniswapV2Pair.sol` (same selectors as Uniswap V2)

## Verification

```bash
./build.sh
# Produces uniswap-v2-factory.polkavm and uniswap-v2-pair.polkavm
```

## Structure

- `src/factory.rs` – UniswapV2Factory
- `src/pair.rs` – UniswapV2Pair + ERC20 LP token
- `src/lib.rs` – Storage helpers, math
- `IUniswapV2Factory.sol`, `IUniswapV2Pair.sol` – Solidity interfaces (ABI source)
