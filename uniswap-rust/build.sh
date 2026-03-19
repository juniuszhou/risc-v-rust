#!/usr/bin/env bash
set -euo pipefail
cd "${0%/*}"
cargo build --release
for binary in uniswap-v2-factory uniswap-v2-pair; do
    echo "Creating ${binary}.polkavm..."
    polkatool link --strip --output "${binary}.polkavm" \
        "target/riscv64emac-unknown-none-polkavm/release/${binary}"
done
