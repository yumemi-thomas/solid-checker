//! `solid-typefacts` over a fixed-answer backend. Test double only: it lets
//! process tests drive the real service loop, crash hooks and trace output
//! while no compiler backend exists.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let result = solid_typefacts::run::run(
        &args,
        std::io::BufReader::new(std::io::stdin()),
        std::io::stdout(),
        solid_typefacts::fake::fake_opener(),
        solid_typefacts::trace::from_environment(),
        &solid_typefacts::run::CrashMarkers::from_environment(),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("solid-typefacts: {error}");
            ExitCode::FAILURE
        }
    }
}
