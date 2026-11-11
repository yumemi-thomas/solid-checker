//! ADR 0950: the default build has no what-if checker in it.
//!
//! `--assume` exists only in a build with the cargo feature `what-if` (`make build-what-if`, its own target directory). A default build answers the flag
//! as an unknown argument, and its binary holds none of the what-if code: no flag, environment variable or input reaches a seam that is not compiled. The
//! module is compiled out of a what-if build (`what_if_process.rs` is that build's tests).
#![cfg(not(feature = "what-if"))]

use std::{env, fs, process::Command};

use crate::what_if_support::{
    FIXTURE, WHAT_IF_MARKERS, contains, daemon_pids, daemon_traces, fixture, socket_directory,
};

const BINARY: &str = env!("CARGO_BIN_EXE_solid-checker-rust");

#[test]
fn the_default_build_answers_assume_as_an_unknown_argument() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let tsconfig = fixture(FIXTURE).join("tsconfig.json");
    for assume in [
        ["--assume", "RootPatch"],
        ["--assume", "OpenMemberWrite(*)"],
        ["--assume", ""],
    ] {
        let output = Command::new(BINARY)
            .env("SOLID_TYPEFACTS_BIN", &typefacts)
            .env("SOLID_CHECKER_DAEMON", "0")
            .args(assume)
            .args(["--format", "json", "--project"])
            .arg(&tsconfig)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{assume:?}: {stderr}");
        assert!(
            output.stdout.is_empty(),
            "{assume:?}: a default build wrote a report for `--assume`"
        );
        assert!(
            stderr.contains("unknown argument") && stderr.contains("--assume"),
            "{assume:?}: {stderr}"
        );
    }
    // The spelling with an equals sign is no flag either.
    let output = Command::new(BINARY)
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .env("SOLID_CHECKER_DAEMON", "0")
        .arg("--assume=RootPatch")
        .args(["--format", "json", "--project"])
        .arg(&tsconfig)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown argument"));
}

#[test]
fn the_default_binary_holds_none_of_the_what_if_code() {
    let bytes = fs::read(BINARY).unwrap();
    for marker in WHAT_IF_MARKERS {
        assert!(
            !contains(&bytes, marker),
            "the default binary holds `{marker}`: the what-if code is compiled into it"
        );
    }
}

/// The control of `what_if_process::the_what_if_binary_makes_no_daemon`: asked for the daemon, a default build leaves the traces that test looks for.
#[test]
fn a_default_build_asked_for_the_daemon_leaves_the_traces_the_what_if_build_must_not() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let tsconfig = fixture(FIXTURE).join("tsconfig.json");
    let temp = socket_directory("default");
    let output = Command::new(BINARY)
        .env("SOLID_TYPEFACTS_BIN", &typefacts)
        .env("SOLID_CHECKER_DAEMON", "1")
        .env("SOLID_CHECKER_DAEMON_IDLE_SECS", "60")
        .env("SOLID_CHECKER_TIMINGS", "1")
        // A retained check is shadow-compared in a test's environment (`.cargo/config.toml`), which a daemon is not asked for.
        .env("SOLID_CHECKER_IR_SHADOW", "0")
        .env("TMPDIR", &temp)
        .args(["--format", "json", "--project"])
        .arg(&tsconfig)
        .output()
        .unwrap();
    let traces = daemon_traces(&temp, &tsconfig);
    let listing = format!(
        "{:?}",
        fs::read_dir(&temp)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .collect::<Vec<_>>()
    );
    for pid in daemon_pids(&tsconfig) {
        let _ = Command::new("kill").arg(pid).status();
    }
    fs::remove_dir_all(&temp).ok();
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !traces.is_empty(),
        "the daemon left no socket and no process: the probe sees nothing ({listing}; {})",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("retained-daemon"),
        "the answer was not the daemon's: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
