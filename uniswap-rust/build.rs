fn main() {
    if std::env::var("CARGO_FEATURE_CONTRACTS").is_err() {
        return;
    }
    cargo_pvm_contract_builder::PvmBuilder::new()
        .with_bins(["uniswap-v2-factory", "uniswap-v2-pair"])
        .build();
}
