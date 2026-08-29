//! Regenerates receipt-issued stable-v1 first-party contract bundles.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde::Serialize;
use solid_facts_backend::{BundleSelector, solid1_bundles, solid2_rc3_bundles};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BundleIndex<'a> {
    schema_version: u16,
    format: &'static str,
    contracts: Vec<BundleIndexEntry<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BundleIndexEntry<'a> {
    package: &'a str,
    artifact_case: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<&'a BundleSelector>,
    document: String,
    receipt: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("solid-contract-bundles: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut root = PathBuf::from(".");
    let mut check = false;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or("--root requires a path")?),
            "--check" => check = true,
            "--help" | "-h" => {
                println!("Usage: solid-contract-bundles [--root <repository>] [--check]");
                return Ok(());
            }
            _ => return Err(format!("unknown argument {argument:?}").into()),
        }
    }
    let groups = [
        ("solid-v1", solid1_bundles()?),
        ("solid-v2", solid2_rc3_bundles()?),
    ];
    let count = groups
        .iter()
        .map(|(_, bundles)| bundles.len())
        .sum::<usize>();
    for (dialect, bundles) in &groups {
        for relative in [
            format!("pkg/contracts/bundled/{dialect}"),
            format!("rust/crates/solid-dialect/contracts/{dialect}"),
        ] {
            let directory = root.join(relative);
            if !check {
                fs::create_dir_all(&directory)?;
            }
            let mut entries = Vec::new();
            for bundle in bundles {
                let document = format!("{}.json", bundle.file_stem);
                let receipt = format!("{}.receipt.json", bundle.file_stem);
                write_or_check(&directory.join(&document), &bundle.document, check)?;
                write_or_check(&directory.join(&receipt), &bundle.receipt, check)?;
                entries.push(BundleIndexEntry {
                    package: &bundle.package,
                    artifact_case: &bundle.artifact_case,
                    selector: bundle.selector.as_ref(),
                    document,
                    receipt,
                });
            }
            let index = serde_json::to_vec_pretty(&BundleIndex {
                schema_version: 1,
                format: "solid-checker-package-contract-bundle-index",
                contracts: entries,
            })?;
            let mut index_with_newline = index;
            index_with_newline.push(b'\n');
            write_or_check(
                &directory.join("bundle-index.json"),
                &index_with_newline,
                check,
            )?;
        }
    }
    if check {
        println!("checked {} receipt-issued stable-v1 bundle cases", count);
    } else {
        println!("generated {} receipt-issued stable-v1 bundle cases", count);
    }
    Ok(())
}

fn write_or_check(
    path: &Path,
    expected: &[u8],
    check: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if check {
        let actual = fs::read(path)
            .map_err(|error| format!("{} is missing or unreadable: {error}", path.display()))?;
        if actual != expected {
            return Err(format!("{} is stale; regenerate contract bundles", path.display()).into());
        }
    } else {
        fs::write(path, expected)?;
    }
    Ok(())
}
