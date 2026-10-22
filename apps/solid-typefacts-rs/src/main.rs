//! `solid-typefacts`: serves one project over the Type Facts v3 stdio
//! lifecycle protocol. Mirrors apps/solid-typefacts/main_native.go.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    match solid_typefacts::run::run_stdio(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("solid-typefacts: {error}");
            ExitCode::FAILURE
        }
    }
}
