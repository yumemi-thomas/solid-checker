//! The machine report (`--format json`): Go-compatible pretty JSON, with the
//! position of every finding recorded as it is written, so an answer can
//! carry the findings of one file without being parsed (ADR 0400).

use std::{cell::RefCell, io, rc::Rc, sync::Arc};

use serde::{
    Serialize,
    ser::{SerializeSeq, SerializeStruct},
};
use solid_facts_backend::{Snapshot, SnapshotFinding};

/// Writes into a shared buffer, so the marks can read where the report is.
struct Shared(Rc<RefCell<Vec<u8>>>);

impl io::Write for Shared {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Where Go's `encoding/json` escapes by default (`&`, `<`, `>`, U+2028,
/// U+2029) serde_json writes the raw character. Keeping Go's escapes makes
/// Rust and Go snapshots byte-for-byte interchangeable during the additive
/// migration. Returns the escaped bytes and, for each raw position in
/// `marks` (ascending), its position in them.
fn go_escape(raw: Vec<u8>, marks: &mut [usize]) -> Vec<u8> {
    // Each hit: the raw position, the bytes it spans, its escape. A raw byte
    // sequence is valid UTF-8, so a hit never starts inside another.
    let mut hits: Vec<(usize, usize, &'static [u8])> = Vec::new();
    let mut ascii = memchr::memchr3_iter(b'&', b'<', b'>', &raw).peekable();
    let mut wide = memchr::memchr_iter(0xE2, &raw)
        .filter(|at| {
            raw.get(*at + 1) == Some(&0x80) && matches!(raw.get(*at + 2), Some(0xA8 | 0xA9))
        })
        .peekable();
    loop {
        let take_ascii = match (ascii.peek().copied(), wide.peek().copied()) {
            (None, None) => break,
            (Some(at), Some(other)) => at < other,
            (Some(_), None) => true,
            (None, Some(_)) => false,
        };
        if take_ascii {
            let Some(at) = ascii.next() else { break };
            let escape: &'static [u8] = match raw[at] {
                b'&' => b"\\u0026",
                b'<' => b"\\u003c",
                _ => b"\\u003e",
            };
            hits.push((at, 1, escape));
        } else {
            let Some(at) = wide.next() else { break };
            let escape: &'static [u8] = if raw[at + 2] == 0xA8 {
                b"\\u2028"
            } else {
                b"\\u2029"
            };
            hits.push((at, 3, escape));
        }
    }
    if hits.is_empty() {
        return raw;
    }
    let mut out = Vec::with_capacity(raw.len() + hits.len() * 5);
    let mut copied = 0;
    let mut grown = 0;
    let mut mark = 0;
    for (at, width, escape) in &hits {
        while mark < marks.len() && marks[mark] <= *at {
            marks[mark] += grown;
            mark += 1;
        }
        out.extend_from_slice(&raw[copied..*at]);
        out.extend_from_slice(escape);
        copied = at + width;
        grown += escape.len() - width;
    }
    while mark < marks.len() {
        marks[mark] += grown;
        mark += 1;
    }
    out.extend_from_slice(&raw[copied..]);
    out
}

/// Where each finding sits in a written report, and whom it concerns.
#[derive(Clone, Debug, Default)]
pub(crate) struct FindingIndex {
    /// Just past the `[` that opens the findings array.
    open: usize,
    /// The end of the last finding (the array's closing line follows).
    close: usize,
    /// The byte range of each finding's object, from its `{` to its `}`.
    spans: Vec<(usize, usize)>,
    slots: Vec<Slot>,
}

#[derive(Clone, Debug)]
struct Slot {
    project: bool,
    /// The primary location's path, then every related location's.
    paths: Vec<Arc<str>>,
}

/// `path` as `path.resolve` would spell an absolute one: `.` and empty
/// segments dropped, `..` applied. `None` for a path it cannot settle without
/// the caller's working directory.
fn lexical(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    if !path.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    Some(format!("/{}", parts.join("/")))
}

impl FindingIndex {
    /// The report with only the findings that can matter to `file`: a
    /// project-scoped finding, one with no path, one whose primary or any
    /// related location is `file`, and any whose paths cannot be compared
    /// exactly. Every other field, and every kept finding's bytes, are the
    /// full report's. `None` when the index does not describe `body`.
    pub(crate) fn only_file(&self, body: &[u8], file: &str) -> Option<Vec<u8>> {
        if self.spans.is_empty() {
            return Some(body.to_vec());
        }
        if self.spans.len() != self.slots.len() || self.close > body.len() {
            return None;
        }
        let wanted = lexical(file);
        let concerns = |slot: &Slot| {
            let (Some(wanted), false) = (&wanted, slot.project) else {
                return true;
            };
            slot.paths.is_empty()
                || slot.paths.iter().any(|path| {
                    path.is_empty()
                        || &**path == file
                        || lexical(path).is_none_or(|path| &path == wanted)
                })
        };
        let mut out = Vec::with_capacity(body.len() / 8);
        out.extend_from_slice(&body[..self.open]);
        let mut first = true;
        for ((start, end), slot) in self.spans.iter().zip(&self.slots) {
            if !concerns(slot) {
                continue;
            }
            out.extend_from_slice(if first { b"\n    " } else { b",\n    " });
            out.extend_from_slice(&body[*start..*end]);
            first = false;
        }
        if first {
            // `[]`, as serde writes an empty array.
            out.push(b']');
            out.extend_from_slice(body.get(self.close + 4..)?);
        } else {
            out.extend_from_slice(&body[self.close..]);
        }
        Some(out)
    }
}

pub(crate) struct JsonReport {
    /// The report, without the trailing newline `--format json` adds.
    pub(crate) output: Vec<u8>,
    pub(crate) index: FindingIndex,
}

struct Marked<'a> {
    finding: &'a SnapshotFinding,
    buffer: &'a Rc<RefCell<Vec<u8>>>,
    spans: &'a RefCell<Vec<(usize, usize)>>,
}

impl Serialize for Marked<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // The separator and indentation are written before the value, so the
        // buffer ends where the finding's `{` goes.
        let start = self.buffer.borrow().len();
        let done = self.finding.serialize(serializer)?;
        self.spans
            .borrow_mut()
            .push((start, self.buffer.borrow().len()));
        Ok(done)
    }
}

struct Findings<'a> {
    findings: &'a [SnapshotFinding],
    buffer: &'a Rc<RefCell<Vec<u8>>>,
    spans: &'a RefCell<Vec<(usize, usize)>>,
    open: &'a RefCell<usize>,
}

impl Serialize for Findings<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.findings.len()))?;
        // `[` is written by `serialize_seq`.
        *self.open.borrow_mut() = self.buffer.borrow().len();
        for finding in self.findings {
            sequence.serialize_element(&Marked {
                finding,
                buffer: self.buffer,
                spans: self.spans,
            })?;
        }
        sequence.end()
    }
}

/// [`Snapshot`]'s own serialization, field for field (the destructuring below
/// stops compiling when a field is added), with the findings marked.
struct Report<'a> {
    snapshot: &'a Snapshot,
    buffer: &'a Rc<RefCell<Vec<u8>>>,
    spans: &'a RefCell<Vec<(usize, usize)>>,
    open: &'a RefCell<usize>,
}

impl Serialize for Report<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Snapshot {
            premises,
            feedback_facts,
            status,
            findings,
            metrics,
        } = self.snapshot;
        let mut report = serializer.serialize_struct("Snapshot", 5)?;
        if !premises.is_empty() {
            report.serialize_field("premises", premises)?;
        }
        if !feedback_facts.is_empty() {
            report.serialize_field("feedbackFacts", feedback_facts)?;
        }
        report.serialize_field("status", status)?;
        report.serialize_field(
            "findings",
            &Findings {
                findings,
                buffer: self.buffer,
                spans: self.spans,
                open: self.open,
            },
        )?;
        report.serialize_field("metrics", metrics)?;
        report.end()
    }
}

/// The pretty machine report of `snapshot`, with its findings indexed.
pub(crate) fn go_compatible_report(snapshot: &Snapshot) -> Result<JsonReport, serde_json::Error> {
    let buffer = Rc::new(RefCell::new(Vec::with_capacity(1 << 16)));
    let spans = RefCell::new(Vec::with_capacity(snapshot.findings.len()));
    let open = RefCell::new(0);
    serde_json::to_writer_pretty(
        Shared(Rc::clone(&buffer)),
        &Report {
            snapshot,
            buffer: &buffer,
            spans: &spans,
            open: &open,
        },
    )?;
    let spans = spans.into_inner();
    let slots = snapshot
        .findings
        .iter()
        .map(|finding| Slot {
            project: finding.subject_kind == "project",
            paths: std::iter::once(&finding.primary_location)
                .chain(&finding.related_locations)
                .map(|location| Arc::from(location.path.as_str()))
                .collect(),
        })
        .collect();
    let raw = Rc::try_unwrap(buffer)
        .map(RefCell::into_inner)
        .unwrap_or_else(|shared| shared.borrow().clone());
    // Every position recorded while writing, in the order it was written.
    let mut marks = Vec::with_capacity(2 * spans.len() + 1);
    marks.push(open.into_inner());
    marks.extend(spans.iter().flat_map(|(start, end)| [*start, *end]));
    let output = go_escape(raw, &mut marks);
    let close = if spans.is_empty() {
        0
    } else {
        marks[marks.len() - 1]
    };
    let spans = marks[1..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[start, end]| (*start, *end))
        .collect();
    Ok(JsonReport {
        output,
        index: FindingIndex {
            open: marks[0],
            close,
            spans,
            slots,
        },
    })
}

#[cfg(test)]
mod tests;
