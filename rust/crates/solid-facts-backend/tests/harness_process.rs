//! Proves the process-test harness is armed. Every fixture-driven test in
//! this directory skips silently when `SOLID_TYPEFACTS_BIN` is unset, so
//! without this canary a bare `cargo test` reports success while verifying
//! nothing.

#[test]
fn typefacts_producer_is_armed() {
    assert!(
        std::env::var_os("SOLID_TYPEFACTS_BIN").is_some(),
        "SOLID_TYPEFACTS_BIN is unset, so every fixture-driven process test \
         in this crate skipped instead of running. Arm the harness with \
         `make test-rust` (or export SOLID_TYPEFACTS_BIN=$PWD/bin/solid-typefacts \
         after `make build-typefacts`) before trusting a green run."
    );
}
