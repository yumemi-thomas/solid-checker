//! A real checker `modules` request encodes `moduleGraph` keys in struct order
//! (`imports`, `importPaths`, `packages`), which is not core deterministic
//! order. Go accepts any key order; so must the Rust producer. Both must give
//! the same answer to the same bytes.

mod common;

use ciborium::Value;
use common::{frame, frames, request};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use typefacts::v3::{Operation, Response};

fn go_binary() -> Option<PathBuf> {
    let path = std::env::var_os("SOLID_TYPEFACTS_GO_BIN").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin/solid-typefacts"),
        PathBuf::from,
    );
    path.is_file().then_some(path).or_else(|| {
        eprintln!("NOTICE: no Go producer; request-order differential skipped");
        None
    })
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn raw_frame(value: &Value) -> Vec<u8> {
    let mut payload = Vec::new();
    ciborium::into_writer(value, &mut payload).unwrap();
    let mut bytes = u32::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
    bytes.extend(payload);
    bytes
}

fn drive(binary: &Path, project: &str) -> Vec<Response> {
    let modules = Value::Map(vec![
        (text("schema"), Value::Integer(1.into())),
        (text("operation"), text("modules")),
        (text("projectId"), text(project)),
        (text("requestId"), Value::Integer(2.into())),
        (text("generation"), Value::Integer(1.into())),
        (
            text("moduleGraph"),
            Value::Map(vec![
                (text("imports"), Value::Bool(true)),
                (text("importPaths"), Value::Array(vec![])),
                (text("packages"), Value::Bool(true)),
            ]),
        ),
    ]);
    let mut open = request(1, Operation::Open, 1);
    open.project_id = project.to_owned();
    let mut close = request(3, Operation::Close, 1);
    close.project_id = project.to_owned();
    let input = [frame(&open), raw_frame(&modules), frame(&close)].concat();
    let mut child = Command::new(binary)
        .args(["-project", project, "-inline-sources"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&input);
    });
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap();
    frames(&output.stdout)
        .into_iter()
        .skip(1)
        .map(|payload| typefacts::decode(&payload).unwrap())
        .collect()
}

#[test]
fn non_canonical_modules_request_matches_go() {
    let Some(go) = go_binary() else { return };
    let directory = std::env::temp_dir().join(format!("tsrs-order-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("tsconfig.json"),
        r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler"},"include":["*.ts"]}"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("a.ts"),
        "import { b } from \"./b\";\nexport const a = b;\n",
    )
    .unwrap();
    std::fs::write(directory.join("b.ts"), "export const b = 1;\n").unwrap();
    let project = directory.canonicalize().unwrap().join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let go_responses = drive(&go, &project);
    let rust_responses = drive(Path::new(env!("CARGO_BIN_EXE_solid-typefacts")), &project);
    assert_eq!(go_responses.len(), 3);
    assert_eq!(go_responses.len(), rust_responses.len());
    for (go, rust) in go_responses.iter().zip(&rust_responses) {
        assert_eq!((go.ok, &go.error), (rust.ok, &rust.error));
    }
    assert!(go_responses[1].ok, "{:?}", go_responses[1].error);
    let project_modules = |response: &Response| {
        response
            .modules
            .iter()
            .filter(|module| !module.path.starts_with("bundled:"))
            .map(|module| module.path.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        project_modules(&go_responses[1]),
        project_modules(&rust_responses[1])
    );
    assert_eq!(
        go_responses[1].module_imports.len(),
        rust_responses[1].module_imports.len()
    );
    let _ = std::fs::remove_dir_all(&directory);
}
