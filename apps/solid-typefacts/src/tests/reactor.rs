//! The owned reactor (Go: `reactor_test.go`): handles, the open envelope, the
//! frame contract, the ABI's status and buffer rules, and a symlinked project.

use super::lifecycle_request;
use super::project_support::{SMALL_CONFIG, Scratch};
use crate::reactor::{Abi, Reactor, open_project};
use ciborium::Value;
use typefacts::v3::{Operation, Response};

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

/// The deterministic envelope, keys in CBOR length-first order.
fn envelope(project_id: &str, config: Option<&[u8]>) -> Vec<u8> {
    let mut entries = vec![(text("projectId"), text(project_id))];
    if let Some(config) = config {
        entries.push((text("hasConfig"), Value::Bool(true)));
        if !config.is_empty() {
            entries.push((text("config"), Value::Bytes(config.to_vec())));
        }
    }
    entries.sort_by_key(|(key, _)| match key {
        Value::Text(name) => (name.len(), name.clone()),
        _ => unreachable!(),
    });
    let mut bytes = Vec::new();
    ciborium::into_writer(&Value::Map(entries), &mut bytes).unwrap();
    bytes
}

fn frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = (payload.len() as u32).to_le_bytes().to_vec();
    frame.extend_from_slice(payload);
    frame
}

fn request_frame(project_id: &str, id: u64, operation: Operation, generation: u64) -> Vec<u8> {
    let mut request = lifecycle_request(id, operation, generation);
    request.project_id = project_id.to_owned();
    frame(&typefacts::encode(&request).unwrap())
}

fn response_of(frame: &[u8]) -> Response {
    assert_eq!(
        u32::from_le_bytes(frame[..4].try_into().unwrap()) as usize,
        frame.len() - 4
    );
    typefacts::decode(&frame[4..]).unwrap()
}

fn reactor() -> Reactor {
    Reactor::new(open_project)
}

fn project() -> Scratch {
    Scratch::new(&[
        ("tsconfig.json", SMALL_CONFIG),
        ("a.ts", "export const a = 1;\n"),
    ])
}

#[test]
fn the_handshake_is_the_startup_frame() {
    let frame = reactor().handshake_frame().unwrap();
    let payload = crate::wire::handshake_payload().unwrap();
    assert_eq!(frame, self::frame(&payload));
}

#[test]
fn handles_are_issued_in_order_and_never_reused() {
    let scratch = project();
    let mut reactor = reactor();
    let first = reactor.open(&envelope(&scratch.config(), None)).unwrap();
    let second = reactor.open(&envelope(&scratch.config(), None)).unwrap();
    assert_eq!((first, second), (1, 2));
    reactor.close(first).unwrap();
    // Closing twice is a no-op; the closed handle is refused for exchanges.
    reactor.close(first).unwrap();
    assert_eq!(
        reactor.lifecycle(first, &[]).unwrap_err(),
        "handle is closed"
    );
    let third = reactor.open(&envelope(&scratch.config(), None)).unwrap();
    assert_eq!(third, 3);
    // A handle this reactor never issued is refused by every operation.
    for handle in [0, 4] {
        assert_eq!(
            reactor.close(handle).unwrap_err(),
            "handle was never issued by this reactor"
        );
        assert_eq!(
            reactor.lifecycle(handle, &[]).unwrap_err(),
            "handle was never issued by this reactor"
        );
    }
}

#[test]
fn an_exchange_answers_one_frame_with_one_frame() {
    let scratch = project();
    let config = scratch.config();
    let mut reactor = reactor();
    let handle = reactor.open(&envelope(&config, None)).unwrap();
    let open = response_of(
        &reactor
            .lifecycle(handle, &request_frame(&config, 1, Operation::Open, 1))
            .unwrap(),
    );
    assert!(open.ok && open.generation == 1, "{open:?}");
    let sources = response_of(
        &reactor
            .lifecycle(handle, &request_frame(&config, 2, Operation::Sources, 1))
            .unwrap(),
    );
    assert!(sources.ok, "{sources:?}");
    assert!(
        sources
            .sources
            .iter()
            .any(|file| file.path.ends_with("/a.ts") && file.source == b"export const a = 1;\n"),
        "inline sources: {sources:?}"
    );
    // A refused request is still an answered frame, with its error code.
    let mismatch = response_of(
        &reactor
            .lifecycle(
                handle,
                &request_frame("/other/tsconfig.json", 3, Operation::Open, 1),
            )
            .unwrap(),
    );
    assert_eq!(mismatch.error.unwrap().code, "project-mismatch");
}

#[test]
fn the_frame_contract_is_enforced() {
    let scratch = project();
    let mut reactor = reactor();
    let handle = reactor.open(&envelope(&scratch.config(), None)).unwrap();
    assert_eq!(
        reactor.lifecycle(handle, &[1, 2]).unwrap_err(),
        "request frame is shorter than its length prefix"
    );
    assert_eq!(
        reactor.lifecycle(handle, &[5, 0, 0, 0, 1]).unwrap_err(),
        "request frame holds 1 payload bytes, its prefix says 5"
    );
    assert_eq!(
        reactor.lifecycle(handle, &[1, 0, 0, 4]).unwrap_err(),
        "request is 67108865 bytes, limit is 67108864"
    );
    assert!(
        reactor
            .lifecycle(handle, &frame(&[0xff]))
            .unwrap_err()
            .starts_with("decode request: decode deterministic CBOR: ")
    );
}

#[test]
fn the_open_envelope_is_validated() {
    let scratch = project();
    let mut reactor = reactor();
    let refused = |reactor: &mut Reactor, bytes: &[u8]| reactor.open(bytes).unwrap_err();
    assert_eq!(
        refused(&mut reactor, &envelope("tsconfig.json", None)),
        "project identity \"tsconfig.json\" is not a clean absolute path"
    );
    assert_eq!(
        refused(&mut reactor, &envelope("/a/../tsconfig.json", None)),
        "project identity \"/a/../tsconfig.json\" is not a clean absolute path"
    );
    assert_eq!(
        refused(&mut reactor, &envelope("", None)),
        "project identity \"\" is not a clean absolute path"
    );
    let mut stray = Vec::new();
    ciborium::into_writer(
        &Value::Map(vec![
            (text("config"), Value::Bytes(b"{}".to_vec())),
            (text("projectId"), text(&scratch.config())),
        ]),
        &mut stray,
    )
    .unwrap();
    assert_eq!(
        refused(&mut reactor, &stray),
        "open request carries configuration bytes without hasConfig"
    );
    let mut unknown = Vec::new();
    ciborium::into_writer(
        &Value::Map(vec![(text("extra"), Value::Bool(true))]),
        &mut unknown,
    )
    .unwrap();
    assert!(
        refused(&mut reactor, &unknown)
            .starts_with("decode open request: decode deterministic CBOR: ")
    );
    assert!(refused(&mut reactor, &[0xff]).starts_with("decode open request: "));
    // Nothing was issued by any refusal.
    assert_eq!(reactor.open(&envelope(&scratch.config(), None)).unwrap(), 1);
}

#[test]
fn a_failed_project_open_issues_no_handle() {
    let scratch = Scratch::new(&[]);
    let mut reactor = reactor();
    assert_eq!(
        reactor
            .open(&envelope(&scratch.config(), None))
            .unwrap_err(),
        format!(
            "open TypeScript project: parse tsconfig: TS5083: Cannot read file '{}'.",
            scratch.config()
        )
    );
    // A configuration overlay at a logical path that does not exist on disk.
    let project = project();
    let logical = project.path("tsconfig.generated.json");
    let handle = reactor
        .open(&envelope(&logical, Some(SMALL_CONFIG.as_bytes())))
        .unwrap();
    assert_eq!(handle, 1);
    let open = response_of(
        &reactor
            .lifecycle(handle, &request_frame(&logical, 1, Operation::Open, 1))
            .unwrap(),
    );
    assert!(open.ok, "{open:?}");
    assert!(!std::path::Path::new(&logical).exists());
}

#[test]
fn the_abi_reports_status_and_replaces_buffers() {
    let scratch = project();
    let mut abi = Abi::new(open_project);
    assert_eq!(abi.allocate_input(0), 0);
    assert_eq!(abi.output_pointer(), 0);
    assert_eq!(abi.handshake(), 0);
    assert_eq!(abi.output_length() as usize, abi.output().len());
    assert_eq!(
        abi.output()[4..],
        crate::wire::handshake_payload().unwrap()[..]
    );

    let bytes = envelope(&scratch.config(), None);
    assert_ne!(abi.allocate_input(bytes.len() as u32), 0);
    abi.input_mut().copy_from_slice(&bytes);
    assert_eq!(abi.open(), 0);
    assert_eq!(abi.output(), 1_u32.to_le_bytes());

    // The input was consumed: a second open sees an empty envelope.
    assert_eq!(abi.open(), 1);
    assert!(
        String::from_utf8(abi.output().to_vec())
            .unwrap()
            .starts_with("decode open request: ")
    );

    let request = request_frame(&scratch.config(), 1, Operation::Open, 1);
    abi.allocate_input(request.len() as u32);
    abi.input_mut().copy_from_slice(&request);
    assert_eq!(abi.lifecycle(1), 0);
    assert!(response_of(abi.output()).ok);

    assert_eq!(abi.close(1), 0);
    assert!(abi.output().is_empty());
    assert_eq!(abi.close(9), 1);
    assert_eq!(abi.output(), b"handle was never issued by this reactor");
}

/// The wasm build resolves runtime paths through tsrs's own symlink walk
/// because `std::fs::canonicalize` is unsupported there. This pins the answer
/// that walk must equal: a project opened through a symlinked root and one
/// opened through the real root name the same runtime symbol, because the
/// identity hashes the symlink-resolved path.
#[cfg(unix)]
#[test]
fn a_symlinked_project_root_names_the_same_runtime_symbol() {
    use super::project_support::{entities, find};
    use crate::context::Context;
    use crate::project::Project;
    use typefacts::v3::EntityDemand;

    let source = "export const value = 1;\n";
    let scratch = Scratch::new(&[("tsconfig.json", SMALL_CONFIG), ("a.ts", source)]);
    let link = std::env::temp_dir().join(format!("solid-typefacts-rs-link-{}", std::process::id()));
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&scratch.root, &link).unwrap();
    let linked_config = link.join("tsconfig.json").to_string_lossy().into_owned();
    let linked_file = link.join("a.ts").to_string_lossy().into_owned();
    let identity = |config: &str, file: &str| {
        let mut project = Project::open(&Context::new(), config, None, None).unwrap();
        let demand = EntityDemand {
            location: find(file, source, "value"),
            symbol: true,
            runtime_identity: true,
            ..EntityDemand::default()
        };
        entities(&mut project, file, &[demand])
            .remove(0)
            .runtime_identity
    };
    let real = identity(&scratch.config(), &scratch.path("a.ts"));
    let linked = identity(&linked_config, &linked_file);
    let _ = std::fs::remove_file(&link);
    assert!(real.starts_with("runtime:h:"), "{real}");
    assert_eq!(real, linked);
}
