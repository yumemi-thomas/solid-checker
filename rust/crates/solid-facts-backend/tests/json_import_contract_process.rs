//! Regression coverage for a `.json` import reaching the native contract
//! generator.
//!
//! `@solidjs/start@2.0.3`'s `dist/shared/dev-toolbar/index.jsx` imports its
//! own `package.json`. The path is not an advertised export-map subpath --
//! it enters analysis only because a runtime source file imports it -- and
//! before the fix, `solid_facts::ast::extract` handed it to Oxc's
//! `SourceType::from_path`, which has no JSON source kind and made the whole
//! generation fail with a fatal AST facts error. This file is a fresh
//! integration test rather than an addition to `contracts_process.rs`
//! because that file already carries unrelated uncommitted work.
//!
//! The adjacent "still fails closed for a genuinely unsupported extension"
//! claim is pinned at the layer that actually owns it instead of here:
//! `solid_facts::ast::tests::a_genuinely_unsupported_extension_still_fails_closed`
//! and `solid_facts_backend::tests::a_non_json_unsupported_extension_still_fails_the_native_build`.
//! Both were tried as a CLI-level fixture first, but TypeScript's own module
//! resolution already refuses to resolve a bare import of an arbitrary asset
//! extension such as `.svg` (there is no `resolveJsonModule`-style carve-out
//! for it) -- confirmed empirically even when the file is named directly as
//! an explicit `files` root -- so that source never reaches
//! `solid_facts::ast::extract` through the ordinary `--project` path at all.
//! A CLI-level test for that claim would either pass vacuously or depend on
//! producer-internal resolution behavior this fix does not control, so the
//! two direct unit tests are the honest place to pin it.

use std::{env, fs, path::PathBuf, process::Command};

use crate::process_support::temporary_directory;

fn expanded_contract(path: &std::path::Path) -> serde_json::Value {
    serde_json::to_value(solid_facts_backend::read_package_contract(path).unwrap()).unwrap()
}

/// The positive case: a package whose ESM entrypoint imports a `.json` file
/// must still generate a contract, with the JSON import itself contributing
/// no callback, no accessor, and no reactive claim -- `packageName`'s return
/// is an ordinary member read and `packageVersion` is a plain value.
#[test]
fn package_generator_certifies_a_json_import_as_inert() {
    let typefacts = match env::var("SOLID_TYPEFACTS_BIN") {
        Ok(value) => value,
        Err(_) => return,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let package = root.join("fixtures/package-contracts/json-import");
    let directory = temporary_directory("json-import-contract");
    let output = directory.join("solid-reactivity.json");
    let result = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .args(["--project"])
        .arg(package.join("tsconfig.json"))
        .args(["--emit-contract"])
        .arg(&output)
        .args([
            "--package-name",
            "json-import-package",
            "--package-version",
            "1.0.0",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "contract generation must certify a JSON import as inert, not fail: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let contract = expanded_contract(&output);
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["packageName"]["kind"], "function",
        "the function reading the JSON import must still be classified normally"
    );
    assert!(
        contract["entrypoints"]["."]["exports"]["packageName"]["callbacks"].is_null(),
        "a plain member read of a JSON import is not a callback obligation"
    );
    assert_eq!(
        contract["entrypoints"]["."]["exports"]["packageVersion"]["kind"], "value",
        "a value re-exported from a JSON import must not be misclassified as reactive"
    );
    fs::remove_dir_all(directory).unwrap();
}
