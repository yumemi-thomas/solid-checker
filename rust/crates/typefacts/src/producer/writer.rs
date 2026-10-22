//! The packed row writer: varints, the transition's shared string dictionary,
//! and the location delta state (Go: `wire_transition_codec.go`,
//! `stringTableV3`).

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::io;
use std::sync::Arc;

use super::{EncodeError, TransitionSink};
use crate::Location;
use crate::v3::{push_uvarint, rows};

/// Rows reach a sink in chunks of at least this many bytes (Go:
/// `streamedWireTransitionChunk`). Chunk boundaries never change the bytes.
pub(super) const STREAMED_CHUNK: usize = 64 << 10;

/// A multiply-rotate string hasher (the FxHash family) for the dictionary.
/// Indexes are assigned in first-occurrence order, so the hash function never
/// reaches the bytes; SipHash's DoS resistance buys nothing for strings the
/// producer itself minted, and its cost showed in the transition encoder.
#[derive(Default)]
pub(super) struct FastHasher(u64);

impl FastHasher {
    const SEED: u64 = 0xf135_7aea_2e62_a9c5;

    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(Self::SEED);
    }
}

impl Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for chunk in chunks {
            self.add(u64::from_le_bytes(*chunk));
        }
        if !rest.is_empty() {
            let mut word = [0_u8; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(word) ^ ((rest.len() as u64) << 56));
        }
    }

    fn write_u8(&mut self, byte: u8) {
        self.add(u64::from(byte));
    }

    fn finish(&self) -> u64 {
        self.0.rotate_left(26)
    }
}

/// Interns strings in first-occurrence order; index 0 is the empty string.
pub(super) struct Dictionary {
    indexes: HashMap<Arc<str>, u64, BuildHasherDefault<FastHasher>>,
    values: Vec<Arc<str>>,
}

impl Dictionary {
    pub(super) fn new() -> Self {
        let empty: Arc<str> = Arc::from("");
        Self {
            indexes: HashMap::from_iter([(empty.clone(), 0)]),
            values: vec![empty],
        }
    }

    pub(super) fn intern(&mut self, value: &str) -> u64 {
        if let Some(index) = self.indexes.get(value) {
            return *index;
        }
        self.insert(Arc::from(value))
    }

    pub(super) fn intern_arc(&mut self, value: &Arc<str>) -> u64 {
        if let Some(index) = self.indexes.get(value.as_ref()) {
            return *index;
        }
        self.insert(value.clone())
    }

    fn insert(&mut self, value: Arc<str>) -> u64 {
        let index = self.values.len() as u64;
        self.indexes.insert(value.clone(), index);
        self.values.push(value);
        index
    }

    pub(super) fn values(&self) -> &[Arc<str>] {
        &self.values
    }
}

/// The previous location of a run: later locations in the run delta-code
/// against it when they share its path.
#[derive(Default)]
pub(super) struct LocationState {
    path: u64,
    start: u64,
    valid: bool,
}

/// Appends varint-coded rows and interns every string into the transition's
/// dictionary. With a sink, full chunks stream out as they fill.
pub(super) struct RowWriter<'s> {
    bytes: Vec<u8>,
    dict: Dictionary,
    sink: Option<&'s mut dyn TransitionSink>,
    flushed: usize,
    error: Option<io::Error>,
}

impl<'s> RowWriter<'s> {
    pub(super) fn new(sink: Option<&'s mut dyn TransitionSink>) -> Self {
        Self {
            bytes: Vec::new(),
            dict: Dictionary::new(),
            sink,
            flushed: 0,
            error: None,
        }
    }

    pub(super) fn u64(&mut self, value: u64) {
        push_uvarint(&mut self.bytes, value);
        self.maybe_flush();
    }

    pub(super) fn bool(&mut self, value: bool) {
        self.u64(u64::from(value));
    }

    pub(super) fn signed(&mut self, value: i64) {
        // Zigzag: the arithmetic shift smears the sign over every bit.
        self.u64(((value as u64) << 1) ^ ((value >> 63) as u64));
    }

    pub(super) fn raw(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
        self.maybe_flush();
    }

    pub(super) fn text(&mut self, value: &str) {
        let index = self.dict.intern(value);
        self.u64(index);
    }

    pub(super) fn text_arc(&mut self, value: &Arc<str>) {
        let index = self.dict.intern_arc(value);
        self.u64(index);
    }

    /// Interns `value` without writing a reference to it, to fix its position
    /// in the dictionary.
    pub(super) fn intern(&mut self, value: &str) -> u64 {
        self.dict.intern(value)
    }

    pub(super) fn location(
        &mut self,
        location: &Location,
        state: &mut LocationState,
    ) -> Result<(), EncodeError> {
        let length = location
            .end_byte
            .checked_sub(location.start_byte)
            .ok_or_else(|| {
                EncodeError::invalid(format!(
                    "end byte {} precedes start byte {}",
                    location.end_byte, location.start_byte
                ))
            })?;
        let path = self.dict.intern_arc(&location.path);
        let start = location.start_byte;
        if state.valid && state.path == path {
            let delta = i64::try_from(i128::from(start) - i128::from(state.start))
                .map_err(|_| EncodeError::invalid("location start delta overflows".to_owned()))?;
            self.u64(1);
            self.signed(delta);
        } else {
            self.u64(path << 1);
            self.u64(start);
        }
        self.u64(length);
        state.path = path;
        state.start = start;
        state.valid = true;
        Ok(())
    }

    pub(super) fn locations(&mut self, locations: &[Location]) -> Result<(), EncodeError> {
        self.u64(locations.len() as u64);
        let mut state = LocationState::default();
        for location in locations {
            self.location(location, &mut state)?;
        }
        Ok(())
    }

    fn maybe_flush(&mut self) {
        if self.sink.is_none() || self.error.is_some() || self.bytes.len() < STREAMED_CHUNK {
            return;
        }
        self.flush();
    }

    fn flush(&mut self) {
        let Some(sink) = self.sink.as_mut() else {
            return;
        };
        let length = self.bytes.len();
        match sink.append_rows(&self.bytes) {
            Ok(()) => self.flushed += length,
            Err(error) => self.error = Some(error),
        }
        self.bytes.clear();
    }

    /// Flushes the tail. Returns the rows still buffered (all of them when
    /// there is no sink) and the dictionary they index.
    pub(super) fn finish(mut self) -> Result<(Vec<u8>, Dictionary), EncodeError> {
        if self.error.is_none() && self.sink.is_some() && !self.bytes.is_empty() {
            self.flush();
        }
        if let Some(error) = self.error {
            return Err(EncodeError::Sink(format!(
                "stream transition rows: {error}"
            )));
        }
        Ok((self.bytes, self.dict))
    }
}

/// Emits hashed symbol IDs as raw digests and every other entry prefix-coded
/// against the preceding non-hashed string.
pub(super) fn append_packed_dictionary(frame: &mut Vec<u8>, dict: &Dictionary) {
    push_uvarint(frame, dict.values().len() as u64);
    let mut previous: &[u8] = b"";
    for value in dict.values() {
        if let Some(digest) = packed_hashed_symbol(value) {
            push_uvarint(frame, rows::STRING_HASHED_SYMBOL);
            frame.extend_from_slice(&digest);
            continue;
        }
        push_uvarint(frame, rows::STRING_PREFIX_CODED);
        let bytes = value.as_bytes();
        let prefix = common_prefix(previous, bytes);
        let suffix = &bytes[prefix..];
        push_uvarint(frame, prefix as u64);
        push_uvarint(frame, suffix.len() as u64);
        frame.extend_from_slice(suffix);
        previous = bytes;
    }
}

/// `symbol:h:` followed by 24 hex digits (either case, as Go's `hex.Decode`
/// reads them) packs to the 12 raw bytes.
fn packed_hashed_symbol(symbol: &str) -> Option<[u8; rows::HASHED_SYMBOL_DIGEST_BYTES]> {
    let digits = symbol.strip_prefix(rows::HASHED_SYMBOL_PREFIX)?;
    if digits.len() != rows::HASHED_SYMBOL_DIGEST_BYTES * 2 {
        return None;
    }
    let mut raw = [0_u8; rows::HASHED_SYMBOL_DIGEST_BYTES];
    for (slot, pair) in raw.iter_mut().zip(digits.as_bytes().as_chunks::<2>().0) {
        *slot = hex_value(pair[0])? << 4 | hex_value(pair[1])?;
    }
    Some(raw)
}

fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

fn common_prefix(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}

/// Go's `wireSymbolName` over the raw bytes of a compiler symbol name.
///
/// TypeScript marks synthetic symbol names with the invalid UTF-8 byte 0xFE;
/// the wire carries their public escaped spelling (`__name`). Any other
/// invalid UTF-8 is repaired the way `strings.ToValidUTF8` does: each run of
/// invalid bytes becomes one U+FFFD. Rust strings are valid by construction,
/// so the encoder takes names as written; a producer whose compiler hands out
/// byte names applies this where it builds the fact.
#[must_use]
pub fn wire_symbol_name_bytes(name: &[u8]) -> String {
    let mut owned;
    let name = match name.strip_prefix(b"\xfe") {
        Some(rest) => {
            owned = b"__".to_vec();
            owned.extend_from_slice(rest);
            owned.as_slice()
        }
        None => name,
    };
    let mut repaired = String::with_capacity(name.len());
    let mut in_run = false;
    for chunk in name.utf8_chunks() {
        if !chunk.valid().is_empty() {
            repaired.push_str(chunk.valid());
            in_run = false;
        }
        if !chunk.invalid().is_empty() && !in_run {
            repaired.push('\u{FFFD}');
            in_run = true;
        }
    }
    repaired
}
