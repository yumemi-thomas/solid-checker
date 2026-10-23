//! Real transitions, from a real compiler, through the Rust encoder.
//!
//! The Go producer (`SOLID_TYPEFACTS_REFERENCE_BIN`, default the archived Go producer)
//! analyzes the repository's TypeScript projects; every table transition it
//! sends (full, delta, sparse, symbol evidence; inline and through its arena)
//! is decoded by the client's decoder and encoded again by this crate's
//! encoder, and the bytes must be identical. Without a Go producer the tests
//! print a notice and pass vacuously, like `differential.rs`.
//!
//! Scope: this proves the row writers, string table, header and dictionary
//! byte for byte on real rows. It cannot prove planning (which operations a
//! given pair of tables yields), because the Rust side has no compiler to build
//! the same tables; planning is covered by the Go goldens and unit tests.
//!
//! `SOLID_TYPEFACTS_CORPUS=all` runs every `fixtures/**/tsconfig.json`; the
//! default is the Go testdata projects plus a deterministic sample.

mod common;

use common::request;
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use typefacts::producer::{TransitionMode, inspect_table_transition, reencode_table_transition};
use typefacts::v3::{EntityDemand, FileChange, Operation, Request, Response, SymbolQuery};
use typefacts::{Location, read_frame};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn go_binary() -> Option<PathBuf> {
    common::reference_binary("transition corpus")
}

struct Producer {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
    project: String,
    next_id: u64,
    arena: Option<PathBuf>,
}

impl Producer {
    fn spawn(binary: &Path, project: &Path, arena: Option<PathBuf>) -> Self {
        let mut command = Command::new(binary);
        command
            .arg("-schema=1")
            .arg("-inline-sources")
            .arg("-project")
            .arg(project);
        if let Some(arena) = &arena {
            std::fs::write(arena, b"").unwrap();
            command.arg(format!("-transition-arena={}", arena.display()));
        }
        let mut child = command
            .env_remove("SOLID_TYPEFACTS_TIMINGS")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let mut output = child.stdout.take().unwrap();
        read_frame(&mut output).expect("handshake");
        Self {
            child,
            input,
            output,
            project: project.to_string_lossy().into_owned(),
            next_id: 1,
            arena,
        }
    }

    /// `None` when the producer died (it is a compiler; some inputs crash it).
    fn ask(
        &mut self,
        operation: Operation,
        generation: u64,
        edit: impl FnOnce(&mut Request),
    ) -> Option<Response> {
        let mut request = request(self.next_id, operation, generation);
        self.next_id += 1;
        request.project_id.clone_from(&self.project);
        edit(&mut request);
        let payload = typefacts::encode(&request).unwrap();
        self.input
            .write_all(&u32::try_from(payload.len()).unwrap().to_le_bytes())
            .ok()?;
        self.input.write_all(&payload).ok()?;
        self.input.flush().ok()?;
        let frame = read_frame(&mut self.output).ok()?;
        match typefacts::decode(&frame) {
            Ok(response) => Some(response),
            Err(error) => {
                // Go can put an unescaped TypeScript-internal (0xFE-prefixed)
                // symbol name into a plain evidence string, which no client
                // can decode. The Rust producer cannot (its strings are valid
                // UTF-8), so such a response is not ours to check.
                eprintln!(
                    "NOTICE: {:?} #{} answered with an undecodable frame: {error}",
                    operation,
                    self.next_id - 1
                );
                None
            }
        }
    }

    /// The transition a response carries, from the response or the arena.
    fn transition(&self, response: &Response) -> Option<Vec<u8>> {
        if !response.table_transition.is_empty() {
            return Some(response.table_transition.clone());
        }
        let [request_id, offset, length] = response.source_lengths[..] else {
            return None;
        };
        assert_eq!(
            Some(Path::new(&response.source_arena)),
            self.arena.as_deref()
        );
        assert_eq!(request_id, response.request_id);
        let mut file = std::fs::File::open(&response.source_arena).unwrap();
        let mut header = [0_u8; 32];
        file.read_exact(&mut header).unwrap();
        assert_eq!(&header[..8], b"STFARENA");
        assert_eq!(
            u64::from_le_bytes(header[8..16].try_into().unwrap()),
            request_id
        );
        assert_eq!(
            u64::from_le_bytes(header[16..24].try_into().unwrap()),
            offset
        );
        assert_eq!(
            u64::from_le_bytes(header[24..32].try_into().unwrap()),
            length
        );
        // Go places the rows at a fixed 64 MiB offset, prefix before them.
        assert!(offset < 64 << 20 && offset + length >= 64 << 20);
        let mut bytes = vec![0; usize::try_from(length).unwrap()];
        file.seek(SeekFrom::Start(offset)).unwrap();
        file.read_exact(&mut bytes).unwrap();
        Some(bytes)
    }
}

impl Drop for Producer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(arena) = &self.arena {
            let _ = std::fs::remove_file(arena);
        }
    }
}

/// Demands every identifier and every simple call span with every fact.
fn demands_for(path: &str, source: &[u8]) -> Vec<EntityDemand> {
    let mut demands = Vec::new();
    let is_start = |byte: u8| byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$';
    let is_part = |byte: u8| is_start(byte) || byte.is_ascii_digit();
    let mut index = 0;
    while index < source.len() && demands.len() < 400 {
        if !is_start(source[index]) {
            index += 1;
            continue;
        }
        let start = index;
        while index < source.len() && is_part(source[index]) {
            index += 1;
        }
        let at = |start: usize, end: usize, call: bool| EntityDemand {
            location: Location {
                path: path.into(),
                start_byte: start as u64,
                end_byte: end as u64,
            },
            symbol: true,
            references: true,
            type_descriptor: true,
            resolved_call: call,
            r#async: true,
            structural_accessor: true,
            callability: true,
            constructability: true,
            runtime_value_domain: true,
            primitive_value_domain: true,
            primitive_literal_candidates: true,
            parameter_object_shape: true,
            call_result_domain: true,
            constant_value: true,
            array_shape: true,
            tuple_shape: true,
            library_types: true,
            reference_space: true,
            runtime_identity: true,
            ..EntityDemand::default()
        };
        let word = std::str::from_utf8(&source[start..index]).unwrap_or("");
        if KEYWORDS.contains(&word) {
            continue;
        }
        demands.push(at(start, index, false));
        if source.get(index) == Some(&b'(') {
            let mut depth = 0_i32;
            for (offset, byte) in source[index..].iter().enumerate() {
                match byte {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            demands.push(at(start, index + offset + 1, true));
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    demands
}

#[derive(Default)]
struct Tally {
    projects: usize,
    skipped: usize,
    frames: BTreeMap<&'static str, usize>,
    bytes: usize,
    rows: usize,
}

impl Tally {
    /// Checks the response's transition (read now: the arena holds only the
    /// latest) and returns its frame.
    fn check(
        &mut self,
        label: &'static str,
        producer: &Producer,
        response: &Response,
    ) -> Option<Vec<u8>> {
        assert!(response.ok, "{label}: {:?}", response.error);
        let frame = producer.transition(response)?;
        let summary = inspect_table_transition(&frame)
            .unwrap_or_else(|error| panic!("{label}: Go's own frame does not decode: {error}"));
        let again = reencode_table_transition(&frame).unwrap();
        assert!(
            again == frame,
            "{label}: re-encoding changed the bytes ({} vs {})",
            again.len(),
            frame.len()
        );
        let key = match (label, summary.mode) {
            ("analyze", TransitionMode::Full) => "analyze full",
            ("analyze", TransitionMode::Delta) => "analyze delta",
            ("symbols", _) => "symbols",
            _ => unreachable!(),
        };
        *self.frames.entry(key).or_default() += 1;
        self.bytes += frame.len();
        self.rows += summary.path_operations + summary.symbol_operations;
        Some(frame)
    }
}

const KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "declare",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "of",
    "private",
    "protected",
    "public",
    "readonly",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "type",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "satisfies",
    "keyof",
    "infer",
    "is",
    "namespace",
    "module",
];

/// Runs one project through the Go producer. `false` when the producer died
/// before the exchange finished (the project is skipped, not failed).
fn exercise(binary: &Path, project: &Path, arena: Option<PathBuf>, tally: &mut Tally) -> bool {
    let mut producer = Producer::spawn(binary, project, arena);
    let Some(sources) = producer
        .ask(Operation::Open, 1, |_| {})
        .and_then(|_| producer.ask(Operation::Sources, 1, |_| {}))
    else {
        return false;
    };
    let root = project.parent().unwrap().to_string_lossy().into_owned();
    let local: Vec<_> = sources
        .sources
        .iter()
        .filter(|file| {
            file.path.starts_with(&root)
                && !file.path.contains("/node_modules/")
                && !file.path.ends_with(".d.ts")
        })
        .collect();
    if local.is_empty() {
        return false;
    }
    let mut demands = Vec::new();
    for file in &local {
        demands.extend(demands_for(&file.path, &file.source));
    }

    let Some(cold) = producer.ask(Operation::Analyze, 1, |r| {
        r.reset_state = true;
        r.demands = demands.clone();
    }) else {
        return false;
    };
    let cold_frame = tally.check("analyze", &producer, &cold);
    let mut token = cold.state_token.clone();

    // Edit the first local file, then analyze again: a delta.
    let edited = local[0];
    let mut source = edited.source.clone();
    source.extend_from_slice(b"\nexport const __probe_for_transition_corpus = 1;\n");
    let Some(updated) = producer.ask(Operation::Update, 2, |r| {
        r.changes = vec![FileChange {
            path: edited.path.clone(),
            source,
            deleted: false,
            version: 2,
        }];
    }) else {
        return false;
    };
    assert!(updated.ok, "{:?}", updated.error);
    let Some(delta) = producer.ask(Operation::Analyze, 2, |r| {
        r.state_token.clone_from(&token);
        r.reference_changes = true;
    }) else {
        return false;
    };
    tally.check("analyze", &producer, &delta);
    token.clone_from(&delta.state_token);

    // Symbol evidence for symbols the first analysis produced.
    if let Some(frame) = cold_frame {
        let ids = inspect_table_transition(&frame).unwrap().entity_symbol_ids;
        let queries: Vec<SymbolQuery> = ids
            .iter()
            .take(40)
            .map(|id| SymbolQuery {
                id: id.as_str().into(),
                references: true,
                references_only: false,
            })
            .collect();
        if !queries.is_empty() {
            let Some(symbols) = producer.ask(Operation::Symbols, 2, |r| {
                r.state_token.clone_from(&token);
                r.symbol_queries = queries;
            }) else {
                return false;
            };
            tally.check("symbols", &producer, &symbols);
        }
    }
    let _ = producer.ask(Operation::Close, 2, |_| {});
    tally.projects += 1;
    true
}

fn corpus() -> Vec<PathBuf> {
    let mut projects: Vec<PathBuf> = Vec::new();
    let testdata = repository().join("apps/solid-typefacts/testdata");
    let mut entries: Vec<_> = std::fs::read_dir(&testdata)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for entry in entries {
        let config = entry.join("tsconfig.json");
        if config.is_file() {
            projects.push(config);
        }
    }
    let mut fixtures = Vec::new();
    collect_configs(&repository().join("fixtures"), &mut fixtures);
    fixtures.sort();
    let all = std::env::var("SOLID_TYPEFACTS_CORPUS").is_ok_and(|value| value == "all");
    let step = if all { 1 } else { 13 };
    projects.extend(fixtures.into_iter().step_by(step));
    projects
}

fn collect_configs(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name() == "node_modules" {
            continue;
        }
        if path.is_dir() {
            collect_configs(&path, found);
        } else if entry.file_name() == "tsconfig.json" {
            found.push(path);
        }
    }
}

fn run_corpus(use_arena: bool) {
    let Some(binary) = go_binary() else { return };
    let mut tally = Tally::default();
    for (index, project) in corpus().iter().enumerate() {
        let arena = use_arena.then(|| {
            std::env::temp_dir().join(format!(
                "solid-typefacts-rs-corpus-{}-{index}.arena",
                std::process::id()
            ))
        });
        if !exercise(&binary, project, arena, &mut tally) {
            tally.skipped += 1;
        }
    }
    eprintln!(
        "transition corpus ({}): {} projects ({} skipped: no local sources or the producer died), frames {:?}, {} bytes, {} operations, every frame re-encoded identically",
        if use_arena { "arena" } else { "inline" },
        tally.projects,
        tally.skipped,
        tally.frames,
        tally.bytes,
        tally.rows
    );
    assert!(tally.projects >= 5, "corpus too small: {}", tally.projects);
    assert!(tally.frames.get("analyze full").copied().unwrap_or(0) >= 5);
    assert!(
        tally.frames.get("analyze delta").copied().unwrap_or(0) >= 1,
        "no delta transition was exercised: {:?}",
        tally.frames
    );
}

#[test]
fn go_transitions_inline_re_encode_identically() {
    run_corpus(false);
}

#[test]
fn go_transitions_through_the_arena_re_encode_identically() {
    run_corpus(true);
}
