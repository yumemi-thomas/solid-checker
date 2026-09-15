//! Authorizes a fixture-supplied contract so a corpus can analyze a consumer
//! against an **accepted** one.
//!
//! This is a gate tool, not a product surface. It is built by the Makefile's
//! checker targets and never packaged: `scripts/assemble-npm-package.mjs` ships
//! `solid-checker-rust` alone.
//!
//! It works on a **copy**. The receipt binds absolute paths — the importer the
//! consumer will itself compute — so an authorized tree is bound to where it
//! sits and cannot be committed. The caller materializes the fixture somewhere
//! scratch, runs this, and analyzes there.
//!
//! ~~~sh
//! solid-contract-authorize --project /scratch/consumer --trust-output /scratch/trust.json
//! solid-checker-rust --project /scratch/consumer/tsconfig.json \
//!   --receipt-trust-configuration /scratch/trust.json --format json
//! ~~~
//!
//! Dropping the second flag is the honest control: the catalog is published and
//! signed, and the analysis still refuses it, because a project cannot nominate
//! its own issuer.

use std::{env, fs, path::PathBuf, process::ExitCode};

use solid_facts_backend::fixture_authorization::{
    authorize_fixture_contract, read_fixture_contract_request,
};

const USAGE: &str = "usage: solid-contract-authorize --project <dir> --trust-output <path>";

fn main() -> ExitCode {
    match run() {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("solid-contract-authorize: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    let mut project: Option<PathBuf> = None;
    let mut trust_output: Option<PathBuf> = None;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--project" => {
                project = Some(PathBuf::from(
                    arguments.next().ok_or("--project needs a directory")?,
                ));
            }
            "--trust-output" => {
                trust_output = Some(PathBuf::from(
                    arguments.next().ok_or("--trust-output needs a path")?,
                ));
            }
            "--help" | "-h" => return Ok(USAGE.to_owned()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    let project = project.ok_or(USAGE)?;
    let trust_output = trust_output.ok_or(USAGE)?;

    // The catalog reader canonicalizes every path it rebases, and the receipt
    // binds the importer it will compute. On macOS `env::temp_dir()` is a
    // symlink (`/var` -> `/private/var`), so an uncanonicalized root here binds
    // a path the consumer never derives.
    let project =
        fs::canonicalize(&project).map_err(|error| format!("{}: {error}", project.display()))?;

    // A trust configuration written *inside* the analyzed project would be
    // inert — the catalog never references trust bytes — and would read as
    // though a project could authorize itself.
    if trust_output.starts_with(&project) {
        return Err(format!(
            "--trust-output {} is inside the project; trust must be supplied out of band",
            trust_output.display()
        ));
    }

    let request = read_fixture_contract_request(&project).map_err(|error| error.to_string())?;
    let authorization =
        authorize_fixture_contract(&project, &request).map_err(|error| error.to_string())?;
    if let Some(parent) = trust_output.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    fs::write(&trust_output, &authorization.trust_configuration)
        .map_err(|error| format!("{}: {error}", trust_output.display()))?;

    Ok(serde_json::json!({
        "document": authorization.document,
        "specifier": authorization.specifier,
        "importer": authorization.importer,
        "issuerScope": authorization.issuer_scope,
        "keyId": authorization.key_id,
        "trustConfiguration": trust_output,
    })
    .to_string())
}
