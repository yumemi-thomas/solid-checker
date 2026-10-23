//! The stdio service loop (main.go `serve`): a reader that keeps frames
//! arriving while the worker computes, one ordered worker, and a writer that
//! encodes and flushes responses.
//!
//! * Requests are computed strictly in arrival order, so a client may pipeline
//!   generation-scoped requests without awaiting intermediate responses.
//! * A `cancel` frame bypasses the queue: the reader fires the target's
//!   [`Context`] at once, and the acknowledgement is ordered like any response.
//! * Every request yields exactly one response frame.
//! * The responder is built, used and dropped on the worker thread, so it need
//!   not be `Send` (a compiler's thread-local state stays on one thread).
//! * A panic on any thread ends the process at once ([`crate::fail_stop`]); no
//!   thread here recovers from one.

use crate::context::Context;
use crate::frame::{FrameError, read_frame, write_frame_header};
use crate::trace::SharedTrace;
use crate::wire::{Incoming, decode_request, encode_response};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use typefacts::v3::{Operation, Response};

/// Answers decoded requests; [`serve`] owns framing, ordering and cancellation.
pub trait Responder {
    fn lifecycle(&mut self, ctx: &Context, incoming: &Incoming) -> Response;
}

struct Job {
    incoming: Incoming,
    ctx: Context,
    request_decode: Duration,
    /// Present for cancellable operations; called after dispatch.
    release: Option<Box<dyn FnOnce() + Send>>,
}

enum Event {
    Frame(Result<Vec<u8>, FrameError>),
    Fatal(String),
}

type Cancels = Arc<Mutex<HashMap<u64, Context>>>;

fn lock(cancels: &Cancels) -> std::sync::MutexGuard<'_, HashMap<u64, Context>> {
    cancels
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Builds the responder on the worker thread, then serves `input` until EOF,
/// a read or decode error, or a write failure.
///
/// `opened` runs on the calling thread once the responder exists and before
/// the first frame is read. A factory error is returned before any frame is
/// read. Returns after every queued request has been answered.
pub fn serve<R, F>(
    make: F,
    opened: impl FnOnce(),
    input: impl Read + Send + 'static,
    output: impl Write + Send + 'static,
    trace: &SharedTrace,
) -> Result<(), String>
where
    R: Responder,
    F: FnOnce() -> Result<R, String> + Send + 'static,
{
    let (jobs_tx, jobs_rx) = mpsc::channel::<Job>();
    let (responses_tx, responses_rx) = mpsc::channel::<Response>();
    let (events_tx, events_rx) = mpsc::channel::<Event>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

    let worker = thread::spawn(move || worker_loop(make, &ready_tx, &jobs_rx, &responses_tx));
    match ready_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(message)) => {
            let _ = worker.join();
            return Err(message);
        }
        Err(_) => {
            let _ = worker.join();
            return Err("worker thread ended before opening".to_owned());
        }
    }
    opened();

    let writer_events = events_tx.clone();
    let writer_trace = trace.clone();
    let writer = thread::spawn(move || {
        writer_loop(output, &responses_rx, &writer_events, writer_trace.as_ref())
    });

    // One frame in flight, like Go's unbuffered channel: the reader takes a
    // credit before each read and main returns it when it receives the frame.
    let (credit_tx, credit_rx) = mpsc::channel::<()>();
    let _ = credit_tx.send(());
    thread::spawn(move || reader_loop(input, &credit_rx, &events_tx));

    let cancels: Cancels = Arc::new(Mutex::new(HashMap::new()));
    let pipeline = Pipeline {
        worker,
        writer,
        events: events_rx,
    };
    loop {
        let Ok(event) = pipeline.events.recv() else {
            return pipeline.drain(jobs_tx, None);
        };
        let frame = match event {
            Event::Fatal(message) => {
                drop(jobs_tx);
                pipeline.join();
                return Err(message);
            }
            Event::Frame(frame) => frame,
        };
        let _ = credit_tx.send(());
        let payload = match frame {
            Ok(payload) => payload,
            Err(FrameError::Eof) => return pipeline.drain(jobs_tx, None),
            Err(error) => return pipeline.drain(jobs_tx, Some(error.to_string())),
        };
        let decode_started = Instant::now();
        let incoming = match decode_request(&payload) {
            Ok(incoming) => incoming,
            Err(error) => {
                return pipeline.drain(jobs_tx, Some(format!("decode request: {error}")));
            }
        };
        drop(payload);
        if incoming.operation() == Some(Operation::Cancel) {
            let target = lock(&cancels)
                .get(&incoming.request.cancel_request_id)
                .cloned();
            if let Some(target) = target {
                target.cancel();
            }
            let _ = jobs_tx.send(Job {
                incoming,
                ctx: Context::new(),
                request_decode: Duration::ZERO,
                release: None,
            });
            continue;
        }
        let request_ctx = Context::new();
        let request_id = incoming.request.request_id;
        lock(&cancels).insert(request_id, request_ctx.clone());
        let release_cancels = Arc::clone(&cancels);
        let release_ctx = request_ctx.clone();
        let _ = jobs_tx.send(Job {
            incoming,
            ctx: request_ctx,
            request_decode: decode_started.elapsed(),
            release: Some(Box::new(move || {
                release_ctx.cancel();
                lock(&release_cancels).remove(&request_id);
            })),
        });
    }
}

struct Pipeline {
    worker: JoinHandle<()>,
    writer: JoinHandle<Option<String>>,
    events: Receiver<Event>,
}

impl Pipeline {
    /// Waits for the worker and writer to finish; returns the writer's final
    /// flush error, if any.
    fn join(self) -> (Option<String>, Receiver<Event>) {
        let _ = self.worker.join();
        let flush = self.writer.join().unwrap_or(None);
        (flush, self.events)
    }

    /// Go's `drain`: close the queue, let the pipeline finish, then report a
    /// write failure first, a read failure second, a failed final flush last.
    fn drain(self, jobs: Sender<Job>, read_error: Option<String>) -> Result<(), String> {
        drop(jobs);
        let (flush, events) = self.join();
        while let Ok(event) = events.try_recv() {
            if let Event::Fatal(message) = event {
                return Err(message);
            }
        }
        if let Some(message) = read_error {
            return Err(message);
        }
        flush.map_or(Ok(()), Err)
    }
}

fn worker_loop<R, F>(
    make: F,
    ready: &Sender<Result<(), String>>,
    jobs: &Receiver<Job>,
    responses: &Sender<Response>,
) where
    R: Responder,
    F: FnOnce() -> Result<R, String>,
{
    let mut responder = match make() {
        Ok(responder) => {
            let _ = ready.send(Ok(()));
            responder
        }
        Err(message) => {
            let _ = ready.send(Err(message));
            return;
        }
    };
    for mut job in jobs {
        let mut response = responder.lifecycle(&job.ctx, &job.incoming);
        // `TSRS_MEM_SPLIT=1`: the thread arenas' size after each request.
        if tsrs_core::memsplit::enabled() {
            tsrs_core::memsplit::report(&format!(
                "after request {} ({:?})",
                job.incoming.request.request_id,
                job.incoming.operation()
            ));
        }
        if let Some(timings) = response.timings.as_mut() {
            timings.request_decode_ns =
                u64::try_from(job.request_decode.as_nanos()).unwrap_or(u64::MAX);
        }
        if let Some(release) = job.release.take() {
            release();
        }
        drop(job);
        let _ = responses.send(response);
    }
}

fn writer_loop(
    mut output: impl Write,
    responses: &Receiver<Response>,
    events: &Sender<Event>,
    trace: Option<&Arc<dyn crate::trace::Trace>>,
) -> Option<String> {
    for response in responses {
        if let Err(message) = write_response(&mut output, &response, trace) {
            let _ = events.send(Event::Fatal(message));
            return None;
        }
    }
    output
        .flush()
        .err()
        .map(|error| crate::gocompat::io_message(&error))
}

fn write_response(
    output: &mut impl Write,
    response: &Response,
    trace: Option<&Arc<dyn crate::trace::Trace>>,
) -> Result<(), String> {
    let encode_started = Instant::now();
    let encoded = encode_response(response)?;
    if let Some(trace) = trace {
        trace.stage("encode-response", encode_started.elapsed());
    }
    let write_started = Instant::now();
    write_frame_header(output, encoded.len())?;
    output
        .write_all(&encoded)
        .and_then(|()| output.flush())
        .map_err(|error| crate::gocompat::io_message(&error))?;
    if let Some(trace) = trace {
        trace.stage("write-response", write_started.elapsed());
    }
    Ok(())
}

fn reader_loop(mut input: impl Read, credits: &Receiver<()>, events: &Sender<Event>) {
    while credits.recv().is_ok() {
        let frame = read_frame(&mut input);
        let finished = frame.is_err();
        if events.send(Event::Frame(frame)).is_err() || finished {
            return;
        }
    }
}
