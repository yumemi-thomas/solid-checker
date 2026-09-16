//! Re-issues a certified, published contract as a bundle this checker compiles
//! in.
//!
//! A maintainer tool, built by the Makefile's checker targets and never
//! packaged. It reads one published `accepted-contracts.json` and the trust
//! configuration its certification wrote, authenticates every entry, and prints
//! the bundles as JSON on stdout — objects included, so the caller decides
//! where they land. `scripts/bundle-accepted-contracts.mjs` is that caller.
//!
//! ~~~sh
//! solid-contract-bundle --catalog <out>/accepted-contracts.json \
//!   --trust-configuration <out>.authority/trust.json
//! ~~~
//!
//! It proves nothing. The certification did; this changes who vouches for the
//! result, from an issuer a consumer must be told to trust to this repository.
//! Every refusal it can emit is in `contract_bundling`.

use std::{env, path::PathBuf, process::ExitCode};

use solid_facts_backend::{bundle_published_catalog, read_policy2_trust_configuration};

const USAGE: &str = "usage: solid-contract-bundle --catalog <path> --trust-configuration <path>";

fn main() -> ExitCode {
    match run() {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("solid-contract-bundle: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    let mut catalog: Option<PathBuf> = None;
    let mut trust: Option<PathBuf> = None;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--catalog" => {
                catalog = Some(PathBuf::from(
                    arguments.next().ok_or("--catalog needs a path")?,
                ));
            }
            "--trust-configuration" => {
                trust = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or("--trust-configuration needs a path")?,
                ));
            }
            "--help" | "-h" => return Ok(USAGE.to_owned()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    let catalog = catalog.ok_or(USAGE)?;
    let trust = trust.ok_or(USAGE)?;
    let trust = read_policy2_trust_configuration(&trust).map_err(|error| error.to_string())?;

    let bundles = bundle_published_catalog(&catalog, &trust)?;
    let objects = bundles
        .iter()
        .flat_map(|bundle| {
            [
                (
                    bundle.entry["document"].clone(),
                    String::from_utf8_lossy(&bundle.document).into_owned(),
                ),
                (
                    bundle.entry["receipt"].clone(),
                    String::from_utf8_lossy(&bundle.receipt).into_owned(),
                ),
            ]
        })
        .map(|(name, bytes)| {
            let name = name.as_str().unwrap_or_default().to_owned();
            (name, serde_json::Value::String(bytes))
        })
        .collect::<serde_json::Map<_, _>>();
    Ok(serde_json::json!({
        "bundles": bundles.iter().map(|bundle| bundle.entry.clone()).collect::<Vec<_>>(),
        "objects": objects,
    })
    .to_string())
}
