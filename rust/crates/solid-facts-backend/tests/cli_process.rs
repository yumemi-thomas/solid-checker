use std::{
    io::Write as _,
    process::{Command, Stdio},
    time::{Duration, SystemTime},
};

#[test]
fn argv_invocation_does_not_wait_for_stdin_eof() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .arg("--help")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Rust CLI with open stdin");
    let stdin = child.stdin.take().expect("keep stdin pipe open");
    let deadline = SystemTime::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll Rust CLI") {
            break status;
        }
        if SystemTime::now() > deadline {
            child.kill().unwrap();
            panic!("Rust CLI blocked on stdin EOF despite argv invocation");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    drop(stdin);
    assert!(status.success());
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
