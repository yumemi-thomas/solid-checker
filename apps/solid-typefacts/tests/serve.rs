//! The service loop against a scripted responder, mirroring main_test.go:
//! arrival-order dispatch, cancel bypass, and the drain/exit semantics.

mod common;

use common::{frame, read_response, request};
use solid_typefacts::context::Context;
use solid_typefacts::serve::{Responder, serve};
use solid_typefacts::wire::{Incoming, blank_response, error_response};
use std::io::{PipeReader, PipeWriter, Write};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use typefacts::v3::{Operation, Response};

struct Ordered {
    generation: u64,
    update_delay: Duration,
    analyze_wait: Duration,
}

impl Responder for Ordered {
    fn lifecycle(&mut self, ctx: &Context, incoming: &Incoming) -> Response {
        let request = &incoming.request;
        let response = blank_response(1, request.request_id, &request.project_id, self.generation);
        match request.operation {
            Operation::Update => {
                std::thread::sleep(self.update_delay);
                if request.generation != self.generation + 1 {
                    return error_response(response, "out-of-order", "update overtaken");
                }
                self.generation += 1;
            }
            Operation::Analyze => {
                let deadline = Instant::now() + self.analyze_wait;
                while Instant::now() < deadline {
                    if ctx.is_cancelled() {
                        return error_response(response, "cancelled", "context canceled");
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                if request.generation != self.generation {
                    return error_response(response, "out-of-order", "analyze overtook update");
                }
            }
            _ => {}
        }
        let mut response = response;
        response.ok = true;
        response
    }
}

struct Served {
    requests: Option<PipeWriter>,
    responses: PipeReader,
    done: JoinHandle<Result<(), String>>,
}

fn start(responder: Ordered) -> Served {
    let (input, requests) = std::io::pipe().unwrap();
    let (responses, output) = std::io::pipe().unwrap();
    let done =
        std::thread::spawn(move || serve(move || Ok(responder), || {}, input, output, &None));
    Served {
        requests: Some(requests),
        responses,
        done,
    }
}

impl Served {
    fn send(&mut self, bytes: &[u8]) {
        self.requests.as_mut().unwrap().write_all(bytes).unwrap();
    }

    fn finish(mut self) -> Result<(), String> {
        drop(self.requests.take());
        self.done.join().unwrap()
    }
}

#[test]
fn pipelined_update_analyze_are_dispatched_in_arrival_order() {
    let mut served = start(Ordered {
        generation: 1,
        update_delay: Duration::from_millis(2),
        analyze_wait: Duration::ZERO,
    });
    let mut writer = served.requests.take().unwrap();
    let sender = std::thread::spawn(move || {
        let mut id = 1;
        for generation in 2..27 {
            writer
                .write_all(&frame(&request(id, Operation::Update, generation)))
                .unwrap();
            id += 1;
            writer
                .write_all(&frame(&request(id, Operation::Analyze, generation)))
                .unwrap();
            id += 1;
        }
    });
    for expected in 1..=50 {
        let response = read_response(&mut served.responses);
        assert_eq!(response.request_id, expected);
        assert!(response.error.is_none(), "{:?}", response.error);
    }
    sender.join().unwrap();
    served.finish().unwrap();
}

#[test]
fn cancel_bypasses_the_ordered_queue() {
    let mut served = start(Ordered {
        generation: 1,
        update_delay: Duration::ZERO,
        analyze_wait: Duration::from_secs(10),
    });
    let started = Instant::now();
    served.send(&frame(&request(7, Operation::Analyze, 1)));
    let mut cancel = request(8, Operation::Cancel, 1);
    cancel.cancel_request_id = 7;
    served.send(&frame(&cancel));
    let analyze = read_response(&mut served.responses);
    assert_eq!(analyze.request_id, 7);
    assert_eq!(
        analyze.error.as_ref().map(|e| e.code.as_str()),
        Some("cancelled")
    );
    assert!(started.elapsed() < Duration::from_secs(5));
    let acknowledgement = read_response(&mut served.responses);
    assert_eq!(acknowledgement.request_id, 8);
    assert!(acknowledgement.ok);
    served.finish().unwrap();
}

#[test]
fn eof_after_queued_work_still_answers_every_request() {
    let mut served = start(Ordered {
        generation: 1,
        update_delay: Duration::from_millis(20),
        analyze_wait: Duration::ZERO,
    });
    served.send(&frame(&request(1, Operation::Update, 2)));
    served.send(&frame(&request(2, Operation::Analyze, 2)));
    let mut responses = served.responses.try_clone().unwrap();
    served.finish().unwrap();
    assert_eq!(read_response(&mut responses).request_id, 1);
    assert_eq!(read_response(&mut responses).request_id, 2);
}

#[test]
fn a_truncated_frame_is_a_read_error_after_the_queue_drains() {
    let mut served = start(Ordered {
        generation: 1,
        update_delay: Duration::ZERO,
        analyze_wait: Duration::ZERO,
    });
    served.send(&frame(&request(1, Operation::Open, 1)));
    served.send(&[9, 0, 0, 0, 1]);
    let mut responses = served.responses.try_clone().unwrap();
    let error = served.finish().unwrap_err();
    assert_eq!(error, "unexpected EOF");
    assert_eq!(read_response(&mut responses).request_id, 1);
}

#[test]
fn an_oversized_frame_and_an_undecodable_frame_end_the_service() {
    let mut served = start(Ordered {
        generation: 1,
        update_delay: Duration::ZERO,
        analyze_wait: Duration::ZERO,
    });
    served.send(&(64_u32 * 1024 * 1024 + 1).to_le_bytes());
    assert_eq!(
        served.finish().unwrap_err(),
        "request is 67108865 bytes, limit is 67108864"
    );

    let mut served = start(Ordered {
        generation: 1,
        update_delay: Duration::ZERO,
        analyze_wait: Duration::ZERO,
    });
    served.send(&[1, 0, 0, 0, 0xff]);
    let error = served.finish().unwrap_err();
    assert!(
        error.starts_with("decode request: decode deterministic CBOR: "),
        "{error}"
    );
}

#[test]
fn a_failing_factory_is_reported_before_any_frame_is_read() {
    let (input, _requests) = std::io::pipe().unwrap();
    let (_responses, output) = std::io::pipe().unwrap();
    let error = serve(
        || -> Result<Ordered, String> { Err("no project".to_owned()) },
        || panic!("opened must not run"),
        input,
        output,
        &None,
    )
    .unwrap_err();
    assert_eq!(error, "no project");
}
