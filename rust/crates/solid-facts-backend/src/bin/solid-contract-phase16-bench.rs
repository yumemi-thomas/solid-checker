//! Emits the checked Phase 16 compactness/load/query benchmark as JSON.

use std::process::ExitCode;

fn main() -> ExitCode {
    match solid_facts_backend::phase16_benchmark_report() {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(encoded) => {
                println!("{encoded}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("solid-contract-phase16-bench: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("solid-contract-phase16-bench: {error}");
            ExitCode::FAILURE
        }
    }
}
