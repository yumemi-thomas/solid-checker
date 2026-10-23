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
/// `~/Documents/Github/sc-worktrees.noindex/tsgo-archive`). They are an
/// acceptance tool for producer changes, not a gate
/// (docs/typefacts/differential.md).
///
/// A reference the caller named (`SOLID_TYPEFACTS_REFERENCE_BIN`), or any
/// reference under `SOLID_TYPEFACTS_REQUIRE_REFERENCE=1`, must be present and
/// speak this client's protocol and schema: otherwise the test fails, never
/// passes vacuously. The implicit archive default is skipped when absent or on
/// another protocol (the archived Go producer speaks 81), with a SKIPPED line
/// written past the test harness's output capture so every run shows it.
///
/// The reference must also be built on the same compiler pin (same tsrs
/// rev, read from its `.buildinfo` stamp): another TypeScript means other lib
/// text, so lib byte offsets and `symbol:h:` ids differ in every row that
/// touches a library declaration. That is the same policy: named fails,
/// implicit skips.
pub fn reference_binary(what: &str) -> Option<std::path::PathBuf> {
    let explicit = std::env::var_os("SOLID_TYPEFACTS_REFERENCE_BIN").map(std::path::PathBuf::from);
    let required =
        std::env::var_os("SOLID_TYPEFACTS_REQUIRE_REFERENCE").is_some_and(|value| value == "1");
    match choose_reference(
        explicit,
        archived_reference(),
        required,
        &reference_handshake,
        &reference_compiler,
    ) {
        Ok(ReferenceChoice::Use(path)) => Some(path),
        Ok(ReferenceChoice::Skip(reason)) => {
            // `eprintln!` is captured for a passing test; the handle is not.
            let line = format!(
                "SKIPPED {what}: {reason} (set SOLID_TYPEFACTS_REFERENCE_BIN, or SOLID_TYPEFACTS_REQUIRE_REFERENCE=1 to fail instead)\n"
            );
            let _ = std::io::stderr().write_all(line.as_bytes());
            None
        }
        Err(reason) => panic!("{what}: {reason}"),
    }
}

/// What [`reference_binary`] does with a candidate reference.
#[derive(Debug, PartialEq, Eq)]
pub enum ReferenceChoice {
    Use(std::path::PathBuf),
    /// Skip the comparison, for this reason.
    Skip(String),
}

/// The decision behind [`reference_binary`], apart from the environment:
/// `explicit` is the reference the caller named, `archive` the implicit
/// default, `required` whether a skip fails instead.
pub fn choose_reference(
    explicit: Option<std::path::PathBuf>,
    archive: Option<std::path::PathBuf>,
    required: bool,
    handshake: &dyn Fn(&std::path::Path) -> Result<typefacts::v3::Handshake, String>,
    compiler: &dyn Fn(&std::path::Path) -> Result<(), String>,
) -> Result<ReferenceChoice, String> {
    let named = explicit.is_some();
    let Some(path) = explicit.or(archive) else {
        return skip_or_fail(required, "no reference producer".to_owned());
    };
    if !path.is_file() {
        let reason = format!("no reference producer at {}", path.display());
        return if named {
            Err(reason)
        } else {
            skip_or_fail(required, reason)
        };
    }
    let handshake = handshake(&path)
        .map_err(|error| format!("reference producer at {}: {error}", path.display()))?;
    if handshake.protocol == typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL
        && handshake.schema_hash == typefacts::v3::TYPE_FACTS_SCHEMA_SHA256
    {
        return match compiler(&path) {
            Ok(()) => Ok(ReferenceChoice::Use(path)),
            Err(reason) if named => Err(format!(
                "reference producer at {}: {reason}",
                path.display()
            )),
            Err(reason) => skip_or_fail(required, reason),
        };
    }
    let reason = format!(
        "reference producer at {} speaks protocol {} (schema {}), this client {}",
        path.display(),
        handshake.protocol,
        handshake.schema_hash,
        typefacts::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL,
    );
    if named {
        Err(reason)
    } else {
        skip_or_fail(required, reason)
    }
}

fn skip_or_fail(required: bool, reason: String) -> Result<ReferenceChoice, String> {
    if required {
        Err(format!("{reason}, and SOLID_TYPEFACTS_REQUIRE_REFERENCE=1"))
    } else {
        Ok(ReferenceChoice::Skip(reason))
    }
}

/// The tsrs rev this producer is pinned to (`apps/solid-typefacts/Cargo.toml`).
fn pinned_tsrs_rev() -> &'static str {
    let manifest = include_str!("../../Cargo.toml");
    let after = manifest
        .split("rev = \"")
        .nth(1)
        .expect("a pinned tsrs rev");
    after.split('"').next().unwrap()
}

/// Whether the producer at `path` was built on the pinned tsrs rev, as its
/// `.buildinfo` stamp (scripts/build-typefacts.sh) records.
fn reference_compiler(path: &std::path::Path) -> Result<(), String> {
    let mut stamp = path.as_os_str().to_owned();
    stamp.push(".buildinfo");
    let stamp = std::path::PathBuf::from(stamp);
    let text = std::fs::read_to_string(&stamp).map_err(|_| {
        format!(
            "no {} stamp, so its compiler pin is unknown",
            stamp.display()
        )
    })?;
    let rev = text
        .split("\"tsrsRev\":\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or("");
    if rev == pinned_tsrs_rev() {
        Ok(())
    } else {
        Err(format!(
            "built on tsrs {rev}, not {}: another TypeScript has other lib text and ids",
            pinned_tsrs_rev()
        ))
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

/// The archived Go producer's path, present or not.
fn archived_reference() -> Option<std::path::PathBuf> {
    let archive = std::env::var_os("TSGO_ARCHIVE")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| {
                std::path::PathBuf::from(home)
                    .join("Documents/Github/sc-worktrees.noindex/tsgo-archive")
            })
        })?;
    Some(archive.join("bin/solid-typefacts-go"))
}
