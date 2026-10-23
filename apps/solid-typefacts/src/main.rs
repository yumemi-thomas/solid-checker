//! `solid-typefacts`: serves one project over the Type Facts v3 stdio
//! lifecycle protocol. Mirrors the retired Go producer's main_native.go.

use std::process::ExitCode;

#[cfg(not(target_arch = "wasm32"))]
#[global_allocator]
static GLOBAL: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

fn main() -> ExitCode {
    // Linux: restore the arena's transparent huge pages after mimalloc's startup
    // constructor disables them (tsrs's own binaries do the same).
    #[cfg(all(target_os = "linux", not(target_arch = "wasm32")))]
    tsrs_core::allocator::init_mimalloc();
    #[cfg(not(target_arch = "wasm32"))]
    solid_typefacts::fail_stop::install();
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
