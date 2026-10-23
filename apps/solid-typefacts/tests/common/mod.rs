#![allow(dead_code)]

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use typefacts::v3::{Operation, Request, Response, TYPE_FACTS_SCHEMA_V1};

pub fn request(id: u64, operation: Operation, generation: u64) -> Request {
    Request {
        schema: TYPE_FACTS_SCHEMA_V1,
        changes: Vec::new(),
        demands: Vec::new(),
        operation,
        project_id: "/p/tsconfig.json".to_owned(),
        request_id: id,
        generation,
        reset_state: false,
        state_token: String::new(),
        compact_demands: None,
        cancel_request_id: 0,
        removed_demand_paths: Vec::new(),
        symbol_queries: Vec::new(),
        release_analysis: false,
        reference_changes: false,
        reference_paths: Vec::new(),
        module_graph: None,
    }
}

pub fn frame(request: &Request) -> Vec<u8> {
    let payload = typefacts::encode(request).unwrap();
    let mut bytes = u32::try_from(payload.len()).unwrap().to_le_bytes().to_vec();
    bytes.extend(payload);
    bytes
}

pub fn read_response(reader: &mut impl Read) -> Response {
    let payload = typefacts::read_frame(reader).unwrap();
    typefacts::decode(&payload).unwrap()
}

/// A `Write + Send + 'static` sink the test can read afterwards.
#[derive(Clone, Default)]
pub struct Sink(pub Arc<Mutex<Vec<u8>>>);

impl Write for Sink {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Sink {
    pub fn bytes(&self) -> Vec<u8> {
        self.0.lock().unwrap().clone()
    }
}

/// Splits a byte stream into frame payloads.
pub fn frames(mut bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    while !bytes.is_empty() {
        let length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        out.push(bytes[4..4 + length].to_vec());
        bytes = &bytes[4 + length..];
    }
    out
}

/// The reference producer the differential tests compare against:
/// `SOLID_TYPEFACTS_REFERENCE_BIN`, else the archived Go producer
/// (`$TSGO_ARCHIVE/bin/solid-typefacts-go`, default
/// `~/Documents/Github/sc-worktrees.noindex/tsgo-archive`). Without one the
/// differential tests print a NOTICE and skip; they are an acceptance tool for
/// producer changes, not a gate (docs/typefacts/differential.md). A reference
/// whose startup handshake names another protocol or schema (the archived Go
/// producer speaks protocol 81) cannot answer this client either: same NOTICE.
pub fn reference_binary(what: &str) -> Option<std::path::PathBuf> {
    let path = reference_path(what)?;
    match reference_handshake(&path) {
        Ok(handshake)
            if handshake.protocol == typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL
                && handshake.schema_hash == typefacts::v3::TYPE_FACTS_SCHEMA_SHA256 =>
        {
            Some(path)
        }
        Ok(handshake) => {
            eprintln!(
                "NOTICE: reference producer at {} speaks protocol {} (schema {}), this client {}; {what} skipped (set SOLID_TYPEFACTS_REFERENCE_BIN)",
                path.display(),
                handshake.protocol,
                handshake.schema_hash,
                typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL,
            );
            None
        }
        Err(error) => panic!("reference producer at {}: {error}", path.display()),
    }
}

/// The startup frame of the producer at `path`.
fn reference_handshake(path: &std::path::Path) -> Result<typefacts::v3::Handshake, String> {
    let mut child = std::process::Command::new(path)
        .args(["-schema=1", "-project", "/nonexistent/tsconfig.json"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let mut output = child.stdout.take().ok_or("no stdout")?;
    let handshake = typefacts::read_frame(&mut output)
        .map_err(|error| error.to_string())
        .and_then(|frame| {
            typefacts::decode::<typefacts::v3::Handshake>(&frame).map_err(|error| error.to_string())
        });
    let _ = child.kill();
    let _ = child.wait();
    handshake
}

fn reference_path(what: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("SOLID_TYPEFACTS_REFERENCE_BIN")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let archive = std::env::var_os("TSGO_ARCHIVE")
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME").map(|home| {
                        std::path::PathBuf::from(home)
                            .join("Documents/Github/sc-worktrees.noindex/tsgo-archive")
                    })
                })?;
            Some(archive.join("bin/solid-typefacts-go"))
        });
    match path {
        Some(path) if path.is_file() => Some(path),
        other => {
            eprintln!(
                "NOTICE: no reference producer{}; {what} skipped (set SOLID_TYPEFACTS_REFERENCE_BIN)",
                other.map_or_else(String::new, |path| format!(" at {}", path.display())),
            );
            None
        }
    }
}
