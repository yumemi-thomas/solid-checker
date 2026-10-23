//! The process-free form of this producer (Go: `reactor.go`): the same
//! session, request decoder and response encoder as the stdio service, driven
//! one synchronous frame at a time by a host that owns the call. It holds
//! several programs (an original and a combined program are separate projects)
//! under owner-issued handles that are never reused within one reactor
//! lifetime.
//!
//! It is not an authority: a handle names a program this reactor opened from
//! the files its host supplied, nothing more.
//!
//! Safe and platform-neutral so the native test suite exercises it; the
//! wasm32-wasip1 exports that wrap it live in `unsafe_boundary`.
//!
//! Go keeps a per-program `busy` flag against a re-entrant exchange. Every
//! operation here takes `&mut self` and runs to completion, so a second
//! exchange cannot begin while one is in flight and the flag would be dead.

use crate::backend::Backend;
use crate::context::Context;
use crate::frame::MAX_MESSAGE_BYTES;
use crate::gocompat::quote;
use crate::session::Session;
use crate::wire::{decode_request, encode_response, handshake_payload};
use ciborium::Value;
use std::collections::HashMap;
use std::path::Path;

const ERR_FOREIGN_HANDLE: &str = "handle was never issued by this reactor";
const ERR_CLOSED_HANDLE: &str = "handle is closed";

/// Opens the program for a project: its clean absolute tsconfig path and, for
/// a host-supplied configuration, its text.
pub type OpenProject = fn(&Context, &str, Option<&str>) -> Result<Box<dyn Backend>, String>;

/// The compiler-backed opener: the same `Project::open` the stdio service uses.
pub fn open_project(
    ctx: &Context,
    project_id: &str,
    config_source: Option<&str>,
) -> Result<Box<dyn Backend>, String> {
    crate::project::Project::open(ctx, project_id, config_source, None)
        .map(|project| Box::new(project) as Box<dyn Backend>)
}

pub struct Reactor {
    open_project: OpenProject,
    next: u32,
    programs: HashMap<u32, Session>,
}

/// The open envelope, deterministic CBOR. Absent and empty configuration
/// differ: `has_config` with empty `config` is an overlay of zero bytes at
/// `project_id`, not the project's own file.
struct OpenRequest {
    project_id: String,
    has_config: bool,
    config: Vec<u8>,
}

impl Reactor {
    #[must_use]
    pub fn new(open_project: OpenProject) -> Self {
        Self {
            open_project,
            next: 0,
            programs: HashMap::new(),
        }
    }

    /// The startup frame the stdio service writes first, length prefix
    /// included.
    pub fn handshake_frame(&self) -> Result<Vec<u8>, String> {
        let payload = handshake_payload()?;
        single_frame(&payload)
    }

    /// Builds one program from the envelope and returns its new handle. On any
    /// failure nothing stays open and no handle is issued.
    pub fn open(&mut self, envelope: &[u8]) -> Result<u32, String> {
        let request = decode_open_request(envelope)?;
        let id = request.project_id.as_str();
        if id.is_empty() || !Path::new(id).has_root() || crate::gocompat::clean(id) != id {
            return Err(format!(
                "project identity {} is not a clean absolute path",
                quote(id)
            ));
        }
        if !request.has_config && !request.config.is_empty() {
            return Err("open request carries configuration bytes without hasConfig".to_owned());
        }
        if self.next == u32::MAX {
            return Err("reactor handle space is exhausted".to_owned());
        }
        let config_source = request
            .has_config
            .then(|| String::from_utf8_lossy(&request.config).into_owned());
        let backend = (self.open_project)(&Context::new(), id, config_source.as_deref())
            .map_err(|error| format!("open TypeScript project: {error}"))?;
        // Session::new owns the backend from here, including on failure.
        let session = Session::new(backend, id, true)?;
        self.next += 1;
        self.programs.insert(self.next, session);
        Ok(self.next)
    }

    /// Answers exactly one request frame with exactly one response frame.
    pub fn lifecycle(&mut self, handle: u32, frame: &[u8]) -> Result<Vec<u8>, String> {
        let session = self.program(handle)?;
        let payload = frame_body(frame)?;
        let incoming =
            decode_request(payload).map_err(|error| format!("decode request: {error}"))?;
        let response = session.lifecycle(&Context::new(), &incoming);
        if tsrs_core::memsplit::enabled() {
            tsrs_core::memsplit::report(&format!("after {:?}", incoming.operation()));
        }
        single_frame(&encode_response(&response)?)
    }

    /// Releases a program. Closing an issued handle twice is a no-op; a handle
    /// this reactor never issued is refused.
    pub fn close(&mut self, handle: u32) -> Result<(), String> {
        if handle == 0 || handle > self.next {
            return Err(ERR_FOREIGN_HANDLE.to_owned());
        }
        match self.programs.remove(&handle) {
            Some(mut session) => session.close(),
            None => Ok(()),
        }
    }

    fn program(&mut self, handle: u32) -> Result<&mut Session, String> {
        if handle == 0 || handle > self.next {
            return Err(ERR_FOREIGN_HANDLE.to_owned());
        }
        self.programs
            .get_mut(&handle)
            .ok_or_else(|| ERR_CLOSED_HANDLE.to_owned())
    }
}

fn codec_error(detail: impl std::fmt::Display) -> String {
    format!("decode open request: decode deterministic CBOR: {detail}")
}

/// Go decodes the envelope with unknown fields and duplicate keys refused,
/// missing fields zero, and a wrong type an error. Key order is not required.
fn decode_open_request(envelope: &[u8]) -> Result<OpenRequest, String> {
    let value: Value = typefacts::decode(envelope)
        .or_else(|_| typefacts::decode_trusted(envelope))
        .map_err(codec_error)?;
    let Value::Map(entries) = value else {
        return Err(codec_error("open request is not a map"));
    };
    let mut request = OpenRequest {
        project_id: String::new(),
        has_config: false,
        config: Vec::new(),
    };
    let mut seen = [false; 3];
    for (key, value) in entries {
        let Value::Text(key) = key else {
            return Err(codec_error("open request key is not text"));
        };
        let slot = match key.as_str() {
            "projectId" => 0,
            "hasConfig" => 1,
            "config" => 2,
            other => {
                return Err(codec_error(format!("unknown field {}", quote(other))));
            }
        };
        if std::mem::replace(&mut seen[slot], true) {
            return Err(codec_error(format!("duplicate map key {}", quote(&key))));
        }
        match (slot, value) {
            (0, Value::Text(text)) => request.project_id = text,
            (1, Value::Bool(flag)) => request.has_config = flag,
            (2, Value::Bytes(bytes)) => request.config = bytes,
            (_, _) => {
                return Err(codec_error(format!(
                    "field {} has the wrong type",
                    quote(&key)
                )));
            }
        }
    }
    Ok(request)
}

/// The payload of exactly one u32-little-endian length-prefixed frame,
/// refusing a short, oversized or trailing frame.
fn frame_body(frame: &[u8]) -> Result<&[u8], String> {
    let Some((prefix, body)) = frame.split_first_chunk::<4>() else {
        return Err("request frame is shorter than its length prefix".to_owned());
    };
    let size = u32::from_le_bytes(*prefix);
    if size as usize > MAX_MESSAGE_BYTES {
        return Err(format!(
            "request is {size} bytes, limit is {MAX_MESSAGE_BYTES}"
        ));
    }
    if body.len() as u64 != u64::from(size) {
        return Err(format!(
            "request frame holds {} payload bytes, its prefix says {size}",
            body.len()
        ));
    }
    Ok(body)
}

/// One payload behind its length prefix, as a single owned buffer.
fn single_frame(payload: &[u8]) -> Result<Vec<u8>, String> {
    if payload.len() > MAX_MESSAGE_BYTES {
        return Err(format!(
            "response is {} bytes, limit is {MAX_MESSAGE_BYTES}",
            payload.len()
        ));
    }
    let size = u32::try_from(payload.len()).map_err(|error| error.to_string())?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&size.to_le_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

/// The reactor's ABI state (Go: `reactor_wasip1.go`). The host writes one
/// input into the buffer [`Abi::allocate_input`] returns, calls one operation,
/// then copies `output_length` bytes from `output_pointer` before its next
/// call: the output buffer is replaced by every operation and must not be
/// retained across calls or memory growth. Every operation returns 0 on
/// success; on failure it returns 1 and the output holds a UTF-8 message.
///
/// * `typefacts_handshake`: output is the startup frame.
/// * `typefacts_open`: input is the CBOR open envelope; output is the new
///   handle as four little-endian bytes.
/// * `typefacts_lifecycle(handle)`: input is one request frame; output is one
///   response frame.
/// * `typefacts_close(handle)`: releases the program; output is empty.
pub struct Abi {
    reactor: Reactor,
    input: Vec<u8>,
    output: Vec<u8>,
}

impl Abi {
    #[must_use]
    pub fn new(open_project: OpenProject) -> Self {
        Self {
            reactor: Reactor::new(open_project),
            input: Vec::new(),
            output: Vec::new(),
        }
    }

    /// Replaces the input with `size` zero bytes and returns their address, or
    /// 0 for none.
    pub fn allocate_input(&mut self, size: u32) -> u32 {
        self.input = vec![0; size as usize];
        address(&self.input)
    }

    /// The input buffer, for a host that is not wasm memory (tests).
    pub fn input_mut(&mut self) -> &mut [u8] {
        &mut self.input
    }

    #[must_use]
    pub fn output_pointer(&self) -> u32 {
        address(&self.output)
    }

    #[must_use]
    pub fn output_length(&self) -> u32 {
        u32::try_from(self.output.len()).unwrap_or(u32::MAX)
    }

    /// The output buffer, for a host that is not wasm memory (tests).
    #[must_use]
    pub fn output(&self) -> &[u8] {
        &self.output
    }

    pub fn handshake(&mut self) -> u32 {
        let result = self.reactor.handshake_frame();
        self.answer(result)
    }

    pub fn open(&mut self) -> u32 {
        let input = std::mem::take(&mut self.input);
        let result = self
            .reactor
            .open(&input)
            .map(|handle| handle.to_le_bytes().to_vec());
        self.answer(result)
    }

    pub fn lifecycle(&mut self, handle: u32) -> u32 {
        let input = std::mem::take(&mut self.input);
        let result = self.reactor.lifecycle(handle, &input);
        self.answer(result)
    }

    pub fn close(&mut self, handle: u32) -> u32 {
        let result = self.reactor.close(handle).map(|()| Vec::new());
        self.answer(result)
    }

    fn answer(&mut self, result: Result<Vec<u8>, String>) -> u32 {
        self.input = Vec::new();
        match result {
            Ok(output) => {
                self.output = output;
                0
            }
            Err(message) => {
                self.output = message.into_bytes();
                1
            }
        }
    }
}

/// A wasm32 linear-memory address (0 for an empty buffer). Only wasm32 has
/// addresses that fit; elsewhere this is a stable truncation for tests.
#[allow(clippy::cast_possible_truncation)]
fn address(buffer: &[u8]) -> u32 {
    if buffer.is_empty() {
        0
    } else {
        buffer.as_ptr() as usize as u32
    }
}
