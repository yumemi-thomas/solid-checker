use std::{
    io::Write as _,
    process::{Command, Stdio},
};

#[test]
fn argv_invocation_ignores_stdin_request() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .arg("--help")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Rust CLI with argv and stdin");
    child
        .stdin
        .take()
        .expect("write stdin payload")
        .write_all(b"this is deliberately not a JSON request")
        .expect("write invalid stdin request");
    let output = child.wait_with_output().expect("wait for Rust CLI help");
    assert!(
        output.status.success(),
        "argv invocation parsed stdin instead: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Usage: solid-checker-rust"),
        "stdout = {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn help_describes_the_dialect_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .arg("--help")
        .output()
        .expect("run Rust CLI help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The flag must be documented; the prose around it is free to change.
    assert!(stdout.contains("--dialect"), "stdout = {stdout}");
}

#[test]
fn argumentless_invocation_accepts_a_json_request_on_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Rust CLI for stdin request");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            br#"{"projectId":"stdin-mode","generation":1,"sources":[],"typefactsExecutable":"","help":true}"#,
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr = {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Usage: solid-checker-rust"),
        "stdout = {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
