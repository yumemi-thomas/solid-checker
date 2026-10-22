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

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::json;
use typefacts::transcript::{decode_request, decode_response, split_frames};
use typefacts::v3::Operation;
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
    let go_answered: HashMap<u64, String> = go_frames
        .iter()
        .skip(1)
        .map(|frame| decode_response(frame).map(|r| (r.request_id, r.state_token)))
        .collect::<Result<_, _>>()?;

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
    for frame in &request_frames {
        if stopped.is_some() {
            break;
        }
        let mut request = decode_request(frame)?;
        if request.operation == Operation::Cancel {
            skipped_cancels += 1;
            continue;
        }
        let mut payload = frame.to_vec();
        if canonicalize {
            payload = typefacts::encode(&request).map_err(|e| e.to_string())?;
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
    let report = json!({
        "requests": request_frames.len(),
        "sent": sent,
        "answered": answered,
        "skippedCancels": skipped_cancels,
        "canonicalized": canonicalize,
        "unmappedStateTokens": unmapped.len(),
        "exitCode": exit_code,
        "stopped": stopped,
        "notes": notes,
    });
    println!("{report}");
    Ok(())
}
