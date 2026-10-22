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
