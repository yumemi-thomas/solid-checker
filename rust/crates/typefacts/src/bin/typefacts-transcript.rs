//! Transcript tool for the Type Facts differential harness.
//!
//!   typefacts-transcript decode <requests.bin> <responses.bin>
//!       Prints the decoded transcript as JSON.
//!   typefacts-transcript replay <requests.bin> <go-responses.bin> <argv.json> <out-responses.bin>
//!       Replays the recorded requests against the producer named by argv[0],
//!       writes its response stream (handshake first) in the recorded format
//!       and prints a JSON replay report.
//!
//! The replay rewrites only `stateToken`: a producer's token is its own text,
//! so each recorded token is mapped to the token this producer returned for the
//! same exchange. Everything else is sent as recorded bytes.
//!
//! A symbol id belongs to the producer that minted it: generation-scoped ids
//! (`symbol:<generation>:<n>`) follow its visit order, and durable ids of
//! library symbols hash declaration offsets of its TypeScript snapshot. A
//! recorded `symbols` request that names one would ask the replaying producer
//! about some other symbol, or none. Like a state token, such an id is
//! translated: it is replaced by the id the replaying producer gave the entity
//! at the same source location (the one the recorded producer gave it, in the
//! preceding analyze tables), when that correspondence is unambiguous; any
//! other id is sent as recorded. The entity tables are still compared as they
//! came, so a symbol that differs is reported there.
//!
//! With `TYPEFACTS_REPLAY_EVIDENCE=<file>` the replay also asks the producer,
//! just before the recorded `close`, for the declarations of every durable
//! symbol id its own tables mentioned (a `symbols` request that is not part of
//! the recorded stream and is not written to the response stream). The rows go
//! to `<file>`. Durable ids hash declaration offsets, so the harness needs a
//! producer's own declarations to prove an id library-only; the recorded
//! streams rarely carry them.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::json;
use typefacts::transcript::{
    IdMap, Tables, decode_request, decode_response, split_frames, symbol_table_rows,
};
use typefacts::v3::{Operation, SymbolQuery};
use typefacts::{encode_sidecar_request, read_frame, write_frame};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.first().map(String::as_str) {
        Some("decode") if arguments.len() == 3 => decode(&arguments[1], &arguments[2]),
        Some("replay") if arguments.len() == 5 => {
            replay(&arguments[1], &arguments[2], &arguments[3], &arguments[4])
        }
        _ => Err("usage: typefacts-transcript decode <req> <res> | replay <req> <go-res> <argv.json> <out>".into()),
    };
    match result {
        Ok(()) => {}
        Err(message) => {
            eprintln!("typefacts-transcript: {message}");
            std::process::exit(2);
        }
    }
}

fn read(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("{path}: {error}"))
}

fn decode(requests: &str, responses: &str) -> Result<(), String> {
    let value = typefacts::transcript::decode_transcript(&read(requests)?, &read(responses)?)?;
    let mut out = std::io::stdout().lock();
    serde_json::to_writer(&mut out, &value).map_err(|error| error.to_string())?;
    writeln!(out).map_err(|error| error.to_string())
}

fn replay(requests: &str, go_responses: &str, argv: &str, output: &str) -> Result<(), String> {
    let request_bytes = read(requests)?;
    let go_bytes = read(go_responses)?;
    let argv: Vec<String> = serde_json::from_slice(&read(argv)?).map_err(|e| e.to_string())?;
    let (program, args) = argv.split_first().ok_or("argv.json is empty")?;
    let timeout = std::env::var("TYPEFACTS_REPLAY_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(Duration::from_secs(120), Duration::from_millis);

    // Diagnostic: send every request in canonical (length-first) key order, to
    // look past a producer that rejects the client's own field order.
    let canonicalize = std::env::var_os("TYPEFACTS_REPLAY_CANONICALIZE").is_some();
    let request_frames = split_frames(&request_bytes)?;
    let go_frames = split_frames(&go_bytes)?;
    let mut go_responses: HashMap<u64, typefacts::v3::Response> = go_frames
        .iter()
        .skip(1)
        .map(|frame| decode_response(frame).map(|r| (r.request_id, r)))
        .collect::<Result<_, _>>()?;
    let go_answered: HashMap<u64, String> = go_responses
        .iter()
        .map(|(id, response)| (*id, response.state_token.clone()))
        .collect();

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn {program}: {error}"))?;
    let mut stdin = child.stdin.take().ok_or("producer stdin unavailable")?;
    let mut stdout = child.stdout.take().ok_or("producer stdout unavailable")?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            let frame = read_frame(&mut stdout).map_err(|error| error.to_string());
            let stop = frame.is_err();
            if sender.send(frame).is_err() || stop {
                break;
            }
        }
        // Drain so a producer blocked on a full pipe can still exit.
        let mut sink = Vec::new();
        let _ = stdout.read_to_end(&mut sink);
    });

    let mut captured: Vec<Vec<u8>> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut skipped_cancels = 0_u64;
    let mut sent = 0_u64;
    let mut answered = 0_u64;
    let mut stopped: Option<String> = None;

    match receiver.recv_timeout(timeout) {
        Ok(Ok(handshake)) => captured.push(handshake),
        Ok(Err(error)) => stopped = Some(format!("no handshake: {error}")),
        Err(_) => stopped = Some("handshake timed out".into()),
    }

    let mut tokens: HashMap<String, String> = HashMap::new();
    let mut unmapped: HashSet<String> = HashSet::new();
    let evidence_path = std::env::var_os("TYPEFACTS_REPLAY_EVIDENCE");
    let mut evidence = Evidence::default();
    let mut go_tables = Tables::default();
    let mut id_map = IdMap::default();
    let mut rewritten_ids = 0_u64;
    let largest_request_id = request_frames
        .iter()
        .filter_map(|frame| decode_request(frame).ok())
        .map(|request| request.request_id)
        .max()
        .unwrap_or(0);
    for frame in &request_frames {
        if stopped.is_some() {
            break;
        }
        let mut request = decode_request(frame)?;
        if request.operation == Operation::Cancel {
            skipped_cancels += 1;
            continue;
        }
        if request.operation == Operation::Close
            && evidence_path.is_some()
            && !evidence.asked
            && stopped.is_none()
        {
            evidence.ask(
                &request,
                largest_request_id + 1_000_000,
                &mut stdin,
                &receiver,
                timeout,
            );
        }
        let mut payload = frame.to_vec();
        if canonicalize {
            payload = typefacts::encode(&request).map_err(|e| e.to_string())?;
        }
        let translated = id_map.translate(&mut request);
        if translated > 0 {
            rewritten_ids += translated;
            payload = if canonicalize {
                typefacts::encode(&request)
            } else {
                encode_sidecar_request(&request)
            }
            .map_err(|e| e.to_string())?;
        }
        if !request.state_token.is_empty() {
            match tokens.get(&request.state_token) {
                Some(mapped) => {
                    if *mapped != request.state_token {
                        request.state_token.clone_from(mapped);
                        payload = if canonicalize {
                            typefacts::encode(&request)
                        } else {
                            encode_sidecar_request(&request)
                        }
                        .map_err(|e| e.to_string())?;
                    }
                }
                None => {
                    unmapped.insert(request.state_token.clone());
                }
            }
        }
        if write_frame(&mut stdin, &payload).is_err() {
            stopped = Some(format!(
                "producer closed stdin at request {}",
                request.request_id
            ));
            break;
        }
        sent += 1;
        let Some(go_token) = go_answered.get(&request.request_id) else {
            continue;
        };
        match receiver.recv_timeout(timeout) {
            Ok(Ok(bytes)) => {
                let response = decode_response(&bytes)?;
                if response.request_id != request.request_id {
                    notes.push(format!(
                        "response id {} for request {}",
                        response.request_id, request.request_id
                    ));
                }
                if !go_token.is_empty() && !response.state_token.is_empty() {
                    tokens.insert(go_token.clone(), response.state_token.clone());
                }
                evidence.observe(request.operation, &response);
                if let Some(recorded) = go_responses.remove(&request.request_id) {
                    go_tables.observe(request.operation, &recorded);
                    if request.operation == Operation::Analyze && response.ok && recorded.ok {
                        id_map.reconcile(&go_tables, &evidence.tables);
                    }
                }
                captured.push(bytes);
                answered += 1;
            }
            Ok(Err(error)) => {
                stopped = Some(format!(
                    "producer stream ended at request {}: {error}",
                    request.request_id
                ));
            }
            Err(_) => {
                stopped = Some(format!("timed out at request {}", request.request_id));
                let _ = child.kill();
            }
        }
    }
    drop(stdin);
    // A producer that already answered everything exits on stdin EOF.
    let mut exit_code = None;
    for _ in 0..200 {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_code = status.code();
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(_) => break,
        }
    }
    if exit_code.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }

    let mut stream = Vec::new();
    for payload in &captured {
        write_frame(&mut stream, payload).map_err(|e| e.to_string())?;
    }
    std::fs::write(output, &stream).map_err(|error| format!("{output}: {error}"))?;
    if let Some(path) = &evidence_path {
        let rendered = serde_json::to_vec(&evidence.render()).map_err(|e| e.to_string())?;
        std::fs::write(path, rendered)
            .map_err(|error| format!("{}: {error}", path.to_string_lossy()))?;
    }
    let report = json!({
        "requests": request_frames.len(),
        "sent": sent,
        "answered": answered,
        "skippedCancels": skipped_cancels,
        "canonicalized": canonicalize,
        "unmappedStateTokens": unmapped.len(),
        "rewrittenSymbolIds": rewritten_ids,
        "exitCode": exit_code,
        "stopped": stopped,
        "notes": notes,
    });
    println!("{report}");
    Ok(())
}

/// The durable ids a producer's own analyze tables mentioned, and what it said
/// about them when asked.
#[derive(Default)]
struct Evidence {
    tables: Tables,
    asked: bool,
    answer: Option<serde_json::Value>,
}

impl Evidence {
    fn observe(&mut self, operation: Operation, response: &typefacts::v3::Response) {
        self.tables.observe(operation, response);
    }

    /// Sends one `symbols` request for every id collected, in place of the
    /// recorded `close` (which the caller still sends afterwards).
    fn ask(
        &mut self,
        close: &typefacts::v3::Request,
        request_id: u64,
        stdin: &mut std::process::ChildStdin,
        receiver: &mpsc::Receiver<Result<Vec<u8>, String>>,
        timeout: Duration,
    ) {
        self.asked = true;
        if self.tables.token.is_empty() || self.tables.ids.is_empty() {
            self.answer = Some(json!({ "requested": 0, "rows": [], "note": "no analyze table" }));
            return;
        }
        let mut request = close.clone();
        request.operation = Operation::Symbols;
        request.request_id = request_id;
        request.state_token.clone_from(&self.tables.token);
        request.symbol_queries = self
            .tables
            .ids
            .iter()
            .map(|id| SymbolQuery {
                id: id.as_str().into(),
                references: false,
                references_only: false,
            })
            .collect();
        let outcome = encode_sidecar_request(&request)
            .map_err(|error| error.to_string())
            .and_then(|payload| write_frame(stdin, &payload).map_err(|error| error.to_string()))
            .and_then(|()| {
                receiver
                    .recv_timeout(timeout)
                    .map_err(|_| "timed out".to_owned())?
            })
            .and_then(|bytes| decode_response(&bytes));
        self.answer = Some(match outcome {
            Ok(response) => {
                let rows = if response.table_transition.is_empty() {
                    Ok(json!(response.symbol_evidence))
                } else {
                    symbol_table_rows(&response)
                };
                match rows {
                    Ok(rows) => json!({
                        "requested": self.tables.ids.len(),
                        "ok": response.ok,
                        "error": response.error.as_ref().map(|e| json!({"code": e.code, "message": e.message})),
                        "rows": rows,
                    }),
                    Err(message) => {
                        json!({ "requested": self.tables.ids.len(), "ok": false, "error": { "code": "evidence-decode", "message": message }, "rows": [] })
                    }
                }
            }
            Err(message) => {
                json!({ "requested": self.tables.ids.len(), "ok": false, "error": { "code": "evidence", "message": message }, "rows": [] })
            }
        });
    }

    fn render(&self) -> serde_json::Value {
        self.answer
            .clone()
            .unwrap_or_else(|| json!({ "requested": 0, "rows": [], "note": "no close request" }))
    }
}
