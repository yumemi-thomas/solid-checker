use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    io::{BufReader, BufWriter, Read},
    path::{Component, Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use thiserror::Error;

use crate::shared_transition_arena::SharedTransitionArena;
use crate::{
    FactTable, ModuleGraph, ModuleGraphDemand, TypeFactsError, decode, decode_trusted,
    encode_sidecar_request, read_frame,
    v3::{
        self, EntityDemand, FileChange, Handshake, ModuleGraphRequest, Operation, Request,
        Response, SlotOp, SourceFile, SymbolQuery, TransitionMode, WireTableTransition,
    },
    write_frame,
};

type PendingResponses = Arc<Mutex<HashMap<u64, mpsc::SyncSender<Result<Response, String>>>>>;

/// An explicitly located Type Facts producer.
///
/// This type intentionally performs no environment or executable-relative
/// lookup. Packaging is a consumer concern.
#[derive(Clone, Debug)]
pub struct Producer {
    path: PathBuf,
    args: Vec<OsString>,
    shared_transition_arena: bool,
}

impl Producer {
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            args: Vec::new(),
            shared_transition_arena: true,
        }
    }

    /// Uses the legacy inline transition payload instead of the default
    /// Rust-owned shared arena.
    #[must_use]
    pub fn without_shared_transition_arena(mut self) -> Self {
        self.shared_transition_arena = false;
        self
    }

    /// Adds producer-specific arguments before the crate-owned `-project`
    /// argument. This is primarily useful for producer diagnostics.
    #[must_use]
    pub fn with_arg(mut self, argument: impl Into<OsString>) -> Self {
        self.args.push(argument.into());
        self
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// The semantic evidence requested for one analysis generation.
#[derive(Clone, Debug, Default)]
pub struct AnalysisDemand {
    pub entities: Vec<EntityDemand>,
}

/// One source file's demand run, borrowed from the caller.
///
/// A caller that already keeps its demands grouped by path — which is the shape
/// any incremental analysis produces — can hand those groups straight to
/// `Session::analyze_groups` without flattening them. The session clones only
/// the groups that actually changed.
///
/// The path is not carried separately: it is read from the demands themselves,
/// so a group cannot disagree with the locations inside it.
#[derive(Clone, Copy, Debug)]
pub struct DemandGroup<'a> {
    demands: &'a [EntityDemand],
    shared: Option<&'a Arc<[EntityDemand]>>,
}

impl<'a> DemandGroup<'a> {
    /// Borrows one file's demand run. Returns `None` for an empty run, which has
    /// no path and therefore no group.
    #[must_use]
    pub fn new(demands: &'a [EntityDemand]) -> Option<Self> {
        if demands.is_empty() {
            return None;
        }
        Some(Self {
            demands,
            shared: None,
        })
    }

    /// Borrows an immutable run that the session may retain by incrementing
    /// its reference count instead of cloning every demand row.
    #[must_use]
    pub fn shared(demands: &'a Arc<[EntityDemand]>) -> Option<Self> {
        if demands.is_empty() {
            return None;
        }
        Some(Self {
            demands,
            shared: Some(demands),
        })
    }

    /// The file every demand in the run belongs to.
    #[must_use]
    pub fn path(&self) -> &'a str {
        &self.demands[0].location.path
    }

    #[must_use]
    pub fn demands(&self) -> &'a [EntityDemand] {
        self.demands
    }

    fn retained(&self) -> Arc<[EntityDemand]> {
        self.shared
            .map_or_else(|| Arc::from(self.demands), Arc::clone)
    }

    /// Reports the first demand whose location leaves this group's file. A
    /// well-formed group has none; `analyze_groups` rejects one that does.
    fn foreign_location(&self) -> Option<&'a str> {
        let path = self.path();
        self.demands
            .iter()
            .map(|demand| demand.location.path.as_ref())
            .find(|candidate: &&str| *candidate != path)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ExchangeTimings {
    pub roundtrip: Duration,
    pub request_send: Duration,
    pub request_bytes: u64,
    pub response_decode: Duration,
    pub response_bytes: u64,
    pub server_request_decode: Duration,
    pub server_analyze: Duration,
    pub server_async: Duration,
    pub server_demand: Duration,
    pub server_assembly: Duration,
    pub server_sort: Duration,
    pub server_close_symbols: Duration,
    pub server_materialized: bool,
    pub server_retained_files: u64,
    pub server_recomputed_files: u64,
    pub server_non_durable_files: u64,
}

/// How long the two halves of one update exchange took.
///
/// `wait` is the part a caller can hide: with `Session::update_during` it is the
/// acknowledgement time left over after the caller's own work finished, so a
/// well-overlapped edit drives it toward zero.
#[derive(Clone, Copy, Debug, Default)]
pub struct UpdateTimings {
    pub send: Duration,
    pub wait: Duration,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TableChanges {
    pub unchanged: bool,
    pub entity_paths: Vec<String>,
    pub symbol_ids: Vec<String>,
    pub file_paths: Vec<String>,
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Type Facts codec or transport error: {0}")]
    TypeFacts(#[from] TypeFactsError),
    #[error("could not start Type Facts producer: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("Type Facts compatibility handshake failed: {0}")]
    Handshake(String),
    #[error("Type Facts process failed: {0}")]
    Process(String),
    #[error("Type Facts service {code}: {message}")]
    Service { code: String, message: String },
    #[error("Type Facts session is closed")]
    Closed,
    #[error("Type Facts response is invalid: {0}")]
    InvalidResponse(String),
    #[error("Type Facts producer provenance is invalid: {0}")]
    ProducerProvenance(String),
}

impl SessionError {
    fn is_transport_failure(&self) -> bool {
        matches!(
            self,
            Self::Process(_) | Self::TypeFacts(TypeFactsError::Io(_))
        )
    }
}

/// A written request awaiting its response. Never escapes the crate.
struct SentRequest {
    request_id: u64,
    receiver: mpsc::Receiver<Result<Response, String>>,
    sent_at: Instant,
    request_send: Duration,
    request_bytes: u64,
    cancellable: bool,
}

/// A producer child process and the threads that serve its pipes.
struct ProcessConnection {
    child: Child,
    writer: Arc<Mutex<BufWriter<ChildStdin>>>,
    pending: PendingResponses,
    next_request_id: Arc<AtomicU64>,
    active_request_id: Arc<AtomicU64>,
    reader: Option<JoinHandle<()>>,
    _transition_arena: Option<Arc<SharedTransitionArena>>,
}

/// A thread-safe handle that asks the producer to cancel the active analysis.
#[derive(Clone)]
pub struct Cancellation {
    writer: Weak<Mutex<BufWriter<ChildStdin>>>,
    next_request_id: Arc<AtomicU64>,
    active_request_id: Arc<AtomicU64>,
    project_id: String,
}

impl Cancellation {
    pub fn cancel_active(&self) -> Result<bool, SessionError> {
        let target = self.active_request_id.load(Ordering::Acquire);
        if target == 0 {
            return Ok(false);
        }
        let Some(writer) = self.writer.upgrade() else {
            return Ok(false);
        };
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let mut request = request(Operation::Cancel, &self.project_id, 1);
        request.request_id = request_id;
        request.cancel_request_id = target;
        let payload = encode_sidecar_request(&request)?;
        let mut writer = writer
            .lock()
            .map_err(|_| SessionError::Process("producer writer is poisoned".into()))?;
        write_frame(&mut *writer, &payload)?;
        Ok(true)
    }
}

impl ProcessConnection {
    fn spawn(producer: &Producer, project_id: &str) -> Result<Self, SessionError> {
        if cfg!(target_family = "wasm") {
            // A WASM build reaches its producer through a FrameBridge, never a
            // child process.
            return Err(SessionError::Spawn(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "process producers are unavailable on this target",
            )));
        }
        let transition_arena = producer
            .shared_transition_arena
            .then(SharedTransitionArena::create)
            .transpose()?
            .map(Arc::new);
        let mut command = Command::new(&producer.path);
        command.args(&producer.args);
        if let Some(arena) = &transition_arena {
            let mut argument = OsString::from("-transition-arena=");
            argument.push(arena.path());
            command.arg(argument);
        }
        let mut child = command
            .args(["-schema=1", "-project", project_id])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(SessionError::Spawn)?;
        let input = child.stdin.take().ok_or_else(|| {
            SessionError::Process("Type Facts producer stdin is unavailable".into())
        })?;
        let output = child.stdout.take().ok_or_else(|| {
            SessionError::Process("Type Facts producer stdout is unavailable".into())
        })?;
        let (handshake_sender, handshake_receiver) = mpsc::sync_channel(1);
        let handshake_reader = std::thread::spawn(move || {
            let mut output = output;
            // The startup frame is the one message read before the producer has
            // proved compatible, so it goes through the deterministic-CBOR
            // validator rather than the trusted fast path.
            let handshake = read_frame(&mut output).and_then(|frame| decode::<Handshake>(&frame));
            let _ = handshake_sender.send((handshake, output));
        });
        // Liveness bound only: the startup frame normally arrives within
        // milliseconds, and compatibility is decided by the frame's content,
        // never by its timing. The bound exists so a hung or dead producer
        // fails eventually rather than blocking forever. It must be generous:
        // under a fully loaded host (the ecosystem benchmark launches one
        // producer per concurrent certification), a freshly spawned process
        // can miss a tight deadline on scheduling alone, and that timeout was
        // observed recording load-dependent *refusals* for packages that
        // certify on an idle machine -- a nondeterministic verdict, which is
        // worse than a slow failure.
        let (handshake, mut output) = match handshake_receiver.recv_timeout(Duration::from_secs(60))
        {
            Ok(result) => {
                handshake_reader
                    .join()
                    .map_err(|_| SessionError::Handshake("startup reader panicked".into()))?;
                result
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                terminate_child(&mut child);
                let _ = handshake_reader.join();
                return Err(SessionError::Handshake(
                    "producer did not report compatibility within 60 seconds".into(),
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                terminate_child(&mut child);
                let _ = handshake_reader.join();
                return Err(SessionError::Handshake(
                    "startup reader disconnected".into(),
                ));
            }
        };
        let handshake = handshake
            .map_err(|error| SessionError::Handshake(format!("invalid startup frame: {error}")))?;
        if let Err(error) = check_handshake(&handshake) {
            terminate_child(&mut child);
            return Err(error);
        }

        let writer = Arc::new(Mutex::new(BufWriter::new(input)));
        let pending = PendingResponses::default();
        let reader_pending = Arc::clone(&pending);
        let reader_arena = transition_arena.as_ref().map(Arc::clone);
        let reader = std::thread::spawn(move || {
            loop {
                let payload = match read_frame(&mut output) {
                    Ok(payload) => payload,
                    Err(error) => {
                        fail_pending(&reader_pending, error.to_string());
                        break;
                    }
                };
                let response = match decode_response_payload(&payload, reader_arena.as_deref()) {
                    Ok(response) => response,
                    Err(error) => {
                        fail_pending(&reader_pending, error);
                        break;
                    }
                };
                if let Ok(mut pending) = reader_pending.lock()
                    && let Some(sender) = pending.remove(&response.request_id)
                {
                    let _ = sender.send(Ok(response));
                }
            }
        });

        Ok(Self {
            child,
            writer,
            pending,
            next_request_id: Arc::new(AtomicU64::new(1)),
            active_request_id: Arc::new(AtomicU64::new(0)),
            reader: Some(reader),
            _transition_arena: transition_arena,
        })
    }

    /// A request written to the producer whose response has not been collected.
    ///
    /// Holding one of these is what lets a caller overlap its own work with an
    /// acknowledgement. It is private: the session decides when a response is
    /// collected, so no consumer can leave one outstanding.
    /// Stamps the request id in place and writes the frame. The request is
    /// borrowed, not consumed: a transport failure lets the caller re-send
    /// the same request over a fresh connection without having cloned it up
    /// front on every exchange.
    fn send(&self, request: &mut Request) -> Result<SentRequest, SessionError> {
        request.request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let request_id = request.request_id;
        let cancellable = matches!(request.operation, Operation::Analyze | Operation::Symbols);
        if cancellable {
            self.active_request_id.store(request_id, Ordering::Release);
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        self.pending
            .lock()
            .map_err(|_| SessionError::Process("pending response map is poisoned".into()))?
            .insert(request_id, sender);
        let sent_at = Instant::now();
        let mut request_bytes = 0;
        let result = (|| {
            let payload = encode_sidecar_request(request)?;
            request_bytes = u64::try_from(payload.len() + 4).unwrap_or(u64::MAX);
            let mut writer = self
                .writer
                .lock()
                .map_err(|_| SessionError::Process("producer writer is poisoned".into()))?;
            write_frame(&mut *writer, &payload)?;
            Ok::<(), SessionError>(())
        })();
        let request_send = sent_at.elapsed();
        if let Err(error) = result {
            if let Ok(mut pending) = self.pending.lock() {
                pending.remove(&request_id);
            }
            if cancellable {
                self.clear_active_request(request_id);
            }
            return Err(error);
        }
        Ok(SentRequest {
            request_id,
            receiver,
            sent_at,
            request_send,
            request_bytes,
            cancellable,
        })
    }

    /// Collects the response to an already-sent request.
    fn wait(&self, sent: SentRequest) -> Result<Response, SessionError> {
        let response = sent
            .receiver
            .recv()
            .map_err(|_| SessionError::Process("producer response channel closed".into()))?
            .map_err(SessionError::Process);
        if sent.cancellable {
            self.clear_active_request(sent.request_id);
        }
        complete_response(response, &sent)
    }

    /// Waits only long enough for the optional graceful-close acknowledgement.
    ///
    /// A producer may exit after failing the preceding request. Its reader
    /// thread can observe EOF and drain the then-current pending map just
    /// before `Session::drop` registers the close request. An unbounded receive
    /// in that race strands the verifier forever because no reader remains to
    /// answer or fail the newly registered request.
    fn wait_for_close(
        &self,
        sent: SentRequest,
        timeout: Duration,
    ) -> Result<Response, SessionError> {
        debug_assert!(!sent.cancellable, "close requests are never cancellable");
        let response = match sent.receiver.recv_timeout(timeout) {
            Ok(response) => response.map_err(SessionError::Process),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Ok(mut pending) = self.pending.lock() {
                    pending.remove(&sent.request_id);
                }
                Err(SessionError::Process(
                    "producer did not acknowledge close before termination".into(),
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(SessionError::Process(
                "producer response channel closed during close".into(),
            )),
        };
        complete_response(response, &sent)
    }

    fn cancellation_handle(&self, project_id: String) -> Cancellation {
        Cancellation {
            writer: Arc::downgrade(&self.writer),
            next_request_id: Arc::clone(&self.next_request_id),
            active_request_id: Arc::clone(&self.active_request_id),
            project_id,
        }
    }

    fn clear_active_request(&self, request_id: u64) {
        self.active_request_id
            .compare_exchange(request_id, 0, Ordering::AcqRel, Ordering::Acquire)
            .ok();
    }

    fn terminate(&mut self) {
        terminate_child(&mut self.child);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for ProcessConnection {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// The carrier of an owned, process-free producer: one program the bridge's
/// owner has already opened, reached one synchronous frame at a time.
///
/// `handshake` returns the producer's startup frame and `exchange` takes
/// exactly one length-prefixed request frame and returns exactly one
/// length-prefixed response frame, the same bytes the stdio service writes.
/// `close` releases the program; it is called exactly once, on every path,
/// including failure. A bridge is no authority: it carries the answers of the
/// producer its owner runs over the files its owner supplied, and it can never
/// issue certification evidence.
pub trait FrameBridge {
    fn handshake(&mut self) -> Result<Vec<u8>, String>;
    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, String>;
    fn close(&mut self);
}

/// The owned producer's open envelope: which program to open, and the bytes
/// overlaid at that logical path when the program is a generated one. Absent
/// and empty configuration differ. Deterministic CBOR, as every frame is.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OwnedOpenRequest<'a> {
    project_id: &'a str,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    has_config: bool,
    #[serde(skip_serializing_if = "<[u8]>::is_empty", with = "serde_bytes")]
    config: &'a [u8],
}

/// Encodes the open envelope an owned reactor's `typefacts_open` reads.
pub fn owned_open_envelope(
    project_id: &str,
    config: Option<&[u8]>,
) -> Result<Vec<u8>, TypeFactsError> {
    crate::encode(&OwnedOpenRequest {
        project_id,
        has_config: config.is_some(),
        config: config.unwrap_or_default(),
    })
}

/// A producer reached through a [`FrameBridge`]. Every answer is ready when
/// `send` returns: there is no overlap, no cancellation, and no recovery. The
/// first transport, decode, or service failure poisons it for good.
struct BridgedConnection {
    bridge: Mutex<Option<Box<dyn FrameBridge + Send>>>,
    next_request_id: AtomicU64,
    poisoned: AtomicBool,
}

impl BridgedConnection {
    fn connect(mut bridge: Box<dyn FrameBridge + Send>) -> Result<Self, SessionError> {
        let handshake = bridge
            .handshake()
            .map_err(SessionError::Handshake)
            .and_then(|frame| single_frame_payload(&frame).map_err(SessionError::Handshake))
            .and_then(|payload| {
                decode::<Handshake>(&payload).map_err(|error| {
                    SessionError::Handshake(format!("invalid startup frame: {error}"))
                })
            })
            .and_then(|handshake| check_handshake(&handshake).map(|()| handshake));
        if let Err(error) = handshake {
            bridge.close();
            return Err(error);
        }
        Ok(Self {
            bridge: Mutex::new(Some(bridge)),
            next_request_id: AtomicU64::new(1),
            poisoned: AtomicBool::new(false),
        })
    }

    fn send(&self, request: &mut Request) -> Result<SentRequest, SessionError> {
        if self.poisoned.load(Ordering::Acquire) {
            return Err(SessionError::Closed);
        }
        request.request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let sent_at = Instant::now();
        let payload = encode_sidecar_request(request)?;
        let mut frame = Vec::with_capacity(payload.len() + 4);
        write_frame(&mut frame, &payload)?;
        let request_bytes = u64::try_from(frame.len()).unwrap_or(u64::MAX);
        let answer = (|| {
            let mut bridge = self
                .bridge
                .lock()
                .map_err(|_| "producer bridge is poisoned".to_owned())?;
            let bridge = bridge
                .as_mut()
                .ok_or_else(|| "producer bridge is closed".to_owned())?;
            let reply = bridge.exchange(&frame)?;
            let payload = single_frame_payload(&reply)?;
            decode_response_payload(&payload, None)
        })();
        if answer.is_err() {
            self.poisoned.store(true, Ordering::Release);
        }
        let request_send = sent_at.elapsed();
        let (sender, receiver) = mpsc::sync_channel(1);
        let _ = sender.send(answer);
        Ok(SentRequest {
            request_id: request.request_id,
            receiver,
            sent_at,
            request_send,
            request_bytes,
            cancellable: false,
        })
    }

    fn wait(&self, sent: SentRequest) -> Result<Response, SessionError> {
        let response = sent
            .receiver
            .try_recv()
            .map_err(|_| SessionError::Process("bridged answer is missing".into()))
            .and_then(|response| response.map_err(SessionError::Process));
        let result = complete_response(response, &sent);
        if result.is_err() {
            self.poisoned.store(true, Ordering::Release);
        }
        result
    }

    fn release(&mut self) {
        if let Ok(mut bridge) = self.bridge.lock()
            && let Some(mut bridge) = bridge.take()
        {
            bridge.close();
        }
    }
}

impl Drop for BridgedConnection {
    fn drop(&mut self) {
        self.release();
    }
}

enum Connection {
    Process(ProcessConnection),
    Bridged(BridgedConnection),
}

impl Connection {
    fn send(&self, request: &mut Request) -> Result<SentRequest, SessionError> {
        match self {
            Self::Process(connection) => connection.send(request),
            Self::Bridged(connection) => connection.send(request),
        }
    }

    fn wait(&self, sent: SentRequest) -> Result<Response, SessionError> {
        match self {
            Self::Process(connection) => connection.wait(sent),
            Self::Bridged(connection) => connection.wait(sent),
        }
    }

    fn wait_for_close(
        &self,
        sent: SentRequest,
        timeout: Duration,
    ) -> Result<Response, SessionError> {
        match self {
            Self::Process(connection) => connection.wait_for_close(sent, timeout),
            Self::Bridged(connection) => connection.wait(sent),
        }
    }

    fn exchange(&self, request: &mut Request) -> Result<Response, SessionError> {
        let sent = self.send(request)?;
        self.wait(sent)
    }

    fn cancellation_handle(&self, project_id: String) -> Option<Cancellation> {
        match self {
            Self::Process(connection) => Some(connection.cancellation_handle(project_id)),
            Self::Bridged(_) => None,
        }
    }

    fn terminate(&mut self) {
        match self {
            Self::Process(connection) => connection.terminate(),
            Self::Bridged(connection) => connection.release(),
        }
    }
}

/// The startup frame must name exactly this client's protocol, schema and
/// build. Compatibility only: it authenticates nothing.
fn check_handshake(handshake: &Handshake) -> Result<(), SessionError> {
    let expected = (
        v3::TYPE_FACTS_HANDSHAKE_PROTOCOL,
        v3::TYPE_FACTS_SCHEMA_SHA256,
        v3::TYPE_FACTS_BUILD_ID,
    );
    let actual = (
        handshake.protocol,
        handshake.schema_hash.as_str(),
        handshake.build_id.as_str(),
    );
    if actual == expected {
        Ok(())
    } else {
        Err(SessionError::Handshake(format!(
            "expected protocol {}, schema {}, build {:?}; got protocol {}, schema {}, build {:?}",
            expected.0, expected.1, expected.2, actual.0, actual.1, actual.2
        )))
    }
}

/// Decodes one response payload exactly as the process reader does,
/// attaching a shared transition arena when the connection has one.
fn decode_response_payload(
    payload: &[u8],
    arena: Option<&SharedTransitionArena>,
) -> Result<Response, String> {
    let decode_started = Instant::now();
    let mut response = decode_trusted::<Response>(payload).map_err(|error| error.to_string())?;
    response.client_response_bytes = u64::try_from(payload.len()).unwrap_or(u64::MAX);
    if let Some(arena) = arena {
        arena
            .attach(&mut response)
            .map_err(|error| error.to_string())?;
    }
    response.client_decode_ns =
        u64::try_from(decode_started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    Ok(response)
}

/// The payload of exactly one length-prefixed frame: no short, oversized or
/// trailing bytes.
fn single_frame_payload(frame: &[u8]) -> Result<Vec<u8>, String> {
    let mut reader = frame;
    let payload = read_frame(&mut reader).map_err(|error| format!("bridged frame: {error}"))?;
    if !reader.is_empty() {
        return Err(format!(
            "bridged frame carries {} trailing bytes",
            reader.len()
        ));
    }
    Ok(payload)
}

/// Finishes one collected answer the same way for every carrier: timings,
/// request correlation, and service errors.
fn complete_response(
    response: Result<Response, SessionError>,
    sent: &SentRequest,
) -> Result<Response, SessionError> {
    let mut response = response?;
    response.client_roundtrip_ns =
        u64::try_from(sent.sent_at.elapsed().as_nanos()).unwrap_or(u64::MAX);
    response.client_request_send_ns =
        u64::try_from(sent.request_send.as_nanos()).unwrap_or(u64::MAX);
    response.client_request_bytes = sent.request_bytes;
    if response.request_id != sent.request_id {
        return Err(SessionError::InvalidResponse(
            "request identity mismatch".into(),
        ));
    }
    if !response.ok {
        let error = response
            .error
            .ok_or_else(|| SessionError::InvalidResponse("error response has no body".into()))?;
        return Err(SessionError::Service {
            code: error.code,
            message: error.message,
        });
    }
    Ok(response)
}

/// A successful answer must be about the program and generation it was asked
/// about.
fn validate_response_identity(
    response: &Response,
    project_id: &str,
    generation: u64,
) -> Result<(), SessionError> {
    if response.schema != v3::TYPE_FACTS_SCHEMA_V1
        || response.project_id != project_id
        || response.generation != generation
    {
        return Err(SessionError::InvalidResponse(format!(
            "answer names schema {}, project {}, generation {}; asked schema {}, project {project_id}, generation {generation}",
            response.schema,
            response.project_id,
            response.generation,
            v3::TYPE_FACTS_SCHEMA_V1
        )));
    }
    Ok(())
}

/// What [`Session::close`] found when it said goodbye.
///
/// Closing is not the same question as whether the producer was still there to
/// hear it, and this separates them. Every variant means the session is closed:
/// `close` marks it closed and terminates the child on every path it can take.
///
/// Deliberately not `#[must_use]`: a caller that simply wants the session shut
/// down is right not to inspect this, and the enclosing `Result` already forces
/// a real failure to be handled. Requiring every `close()` to bind the outcome
/// would add noise to the ~28 call sites that correctly do not care.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseOutcome {
    /// The producer acknowledged the close request.
    Acknowledged,
    /// The producer was already gone -- it had died, or it did not answer
    /// within the bound -- so the child was terminated without a goodbye.
    ///
    /// Not a failure. Nothing is left behind either way: the child is killed
    /// and its reader joined by `Connection::terminate`, and a shared
    /// transition arena is removed by its own `Drop` on this side of the pipe.
    ProducerAlreadyGone,
}

/// A retained Type Facts session.
///
/// Framing, request identities, handshake validation, Wire table transitions,
/// and subprocess recovery are private implementation details.
pub struct Session {
    /// The process to relaunch on recovery. `None` for a bridged session,
    /// which never relaunches, replays, or re-acquires: its first failure
    /// closes it.
    producer: Option<Producer>,
    project_id: String,
    generation: u64,
    restart_epoch: u64,
    connection: Option<Connection>,
    /// Overlays to replay if the producer dies, one entry per accepted update.
    /// A batch is kept even once emptied by `supersede`, because the producer
    /// advances a generation per accepted update and replay must land on the
    /// generation this session already reports.
    replay_batches: Vec<Vec<FileChange>>,
    /// Where each path's newest overlay lives in `replay_batches`, so
    /// superseding an earlier copy costs one lookup rather than a scan.
    replay_index: HashMap<String, usize>,
    state_token: String,
    retained_demands: HashMap<String, Arc<[EntityDemand]>>,
    retained_table: Option<FactTable>,
    affected_paths: HashSet<String>,
    reference_tier: HashSet<Arc<str>>,
    symbols_by_path: HashMap<Arc<str>, Vec<Arc<str>>>,
    last_exchange_timings: Option<ExchangeTimings>,
    last_update_timings: Option<UpdateTimings>,
    last_table_changes: Option<TableChanges>,
    closed: bool,
}

fn normalize_project_id(project_id: String) -> Result<String, SessionError> {
    if project_id.trim().is_empty() {
        return Err(SessionError::InvalidResponse(
            "project identity is empty".into(),
        ));
    }
    let path = PathBuf::from(project_id);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|error| {
                SessionError::InvalidResponse(format!(
                    "could not resolve project identity: {error}"
                ))
            })?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
        .into_os_string()
        .into_string()
        .map_err(|_| SessionError::InvalidResponse("project identity is not valid UTF-8".into()))
}

impl Session {
    /// Opens a session for a project path. Relative and lexically non-canonical
    /// paths are normalized to the same absolute identity used by the producer.
    pub fn open<I>(
        producer: Producer,
        project_id: impl Into<String>,
        sources: I,
    ) -> Result<Self, SessionError>
    where
        I: IntoIterator<Item = FileChange>,
    {
        let project_id = normalize_project_id(project_id.into())?;
        let connection = Connection::Process(ProcessConnection::spawn(&producer, &project_id)?);
        let mut session = Self::with_connection(Some(producer), project_id, connection);
        session.exchange(request(
            Operation::Open,
            &session.project_id,
            session.generation,
        ))?;
        let sources = sources.into_iter().collect::<Vec<_>>();
        if !sources.is_empty() {
            session.update(sources)?;
        }
        Ok(session)
    }

    /// Opens a session over a producer program its owner has already opened
    /// for `project_id` (see [`FrameBridge`]). The answers decode, validate,
    /// and expand exactly as a process session's do. A bridged session is
    /// never relaunched or replayed, has no cancellation handle, and refuses
    /// certification; its first failure closes it and drops its retained
    /// state.
    pub fn open_bridged(
        bridge: Box<dyn FrameBridge + Send>,
        project_id: impl Into<String>,
    ) -> Result<Self, SessionError> {
        let project_id = match normalize_project_id(project_id.into()) {
            Ok(project_id) => project_id,
            Err(error) => {
                let mut bridge = bridge;
                bridge.close();
                return Err(error);
            }
        };
        let connection = Connection::Bridged(BridgedConnection::connect(bridge)?);
        let mut session = Self::with_connection(None, project_id, connection);
        let opened = session.exchange(request(
            Operation::Open,
            &session.project_id,
            session.generation,
        ));
        session.guard(opened)?;
        Ok(session)
    }

    fn with_connection(
        producer: Option<Producer>,
        project_id: String,
        connection: Connection,
    ) -> Self {
        Self {
            producer,
            project_id,
            generation: 1,
            restart_epoch: 0,
            connection: Some(connection),
            replay_batches: Vec::new(),
            replay_index: HashMap::new(),
            state_token: String::new(),
            retained_demands: HashMap::new(),
            retained_table: None,
            affected_paths: HashSet::new(),
            reference_tier: HashSet::new(),
            symbols_by_path: HashMap::new(),
            last_exchange_timings: None,
            last_update_timings: None,
            last_table_changes: None,
            closed: false,
        }
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn cancellation_handle(&self) -> Option<Cancellation> {
        self.connection
            .as_ref()
            .and_then(|connection| connection.cancellation_handle(self.project_id.clone()))
    }

    pub fn take_last_exchange_timings(&mut self) -> Option<ExchangeTimings> {
        self.last_exchange_timings.take()
    }

    pub fn take_last_table_changes(&mut self) -> Option<TableChanges> {
        self.last_table_changes.take()
    }

    /// Analyses one generation from a flat demand list.
    ///
    /// A compatibility shape over `analyze_groups`: the list is grouped by path
    /// first, which costs one clone of the whole demand set. A caller that
    /// already holds its demands grouped should call `analyze_groups` and skip
    /// that entirely.
    pub fn analyze(&mut self, demand: &AnalysisDemand) -> Result<FactTable, SessionError> {
        let owned = group_demands(&demand.entities);
        let groups = owned
            .iter()
            .filter_map(|run| DemandGroup::new(run))
            .collect::<Vec<_>>();
        self.analyze_groups(&groups)
    }

    /// Returns the most recent update's send and wait split, if there was one.
    pub fn take_last_update_timings(&mut self) -> Option<UpdateTimings> {
        self.last_update_timings.take()
    }

    /// Analyses one generation from demands the caller already keeps grouped by
    /// path.
    ///
    /// This is the canonical analysis entry point, and the cheap one. A group
    /// equal to the retained state costs one lookup and one slice comparison —
    /// it is never cloned and never transmitted. Only groups that actually
    /// changed are cloned, so per-edit allocation tracks the number of changed
    /// groups rather than the size of the demand set.
    ///
    /// `analyze` is a thin wrapper that groups a flat list and calls through to
    /// here, so there is one retained-analysis implementation rather than two.
    pub fn analyze_groups(
        &mut self,
        groups: &[DemandGroup<'_>],
    ) -> Result<FactTable, SessionError> {
        let result = self.analyze_groups_unguarded(groups);
        self.guard(result)
    }

    fn analyze_groups_unguarded(
        &mut self,
        groups: &[DemandGroup<'_>],
    ) -> Result<FactTable, SessionError> {
        self.ensure_open()?;

        // Group paths, used both to reject duplicates and to find retained paths
        // the caller has dropped. Borrowed, so naming 1,000 groups allocates one
        // set of string references rather than 1,000 strings.
        let mut present = HashSet::with_capacity(groups.len());
        for group in groups {
            if !present.insert(group.path()) {
                return Err(SessionError::InvalidResponse(format!(
                    "demand groups name {} twice; each path may appear once",
                    group.path()
                )));
            }
        }

        let reset_state = self.state_token.is_empty();
        let mut changed = Vec::new();
        let mut removed = Vec::new();
        if reset_state {
            changed.extend_from_slice(groups);
        } else {
            for group in groups {
                let unchanged = self
                    .retained_demands
                    .get(group.path())
                    .is_some_and(|retained| retained.as_ref() == group.demands());
                if !unchanged {
                    changed.push(*group);
                }
            }
            removed = self
                .retained_demands
                .keys()
                .filter(|path| !present.contains(path.as_str()))
                .cloned()
                .collect();
            removed.sort();
        }

        // Only what will be transmitted or newly retained needs checking; an
        // unchanged group was validated when it was first retained.
        Self::reject_foreign_locations(&changed)?;
        // Path order fixes the request bytes for a given set of changed groups.
        changed.sort_by_key(|group| group.path());

        let wire_demands = changed
            .iter()
            .flat_map(|group| group.demands().iter().cloned())
            .collect::<Vec<_>>();
        let reference_locations = groups
            .iter()
            .flat_map(|group| group.demands())
            .filter(|demand| demand.references)
            .map(|demand| demand.location.clone())
            .collect::<HashSet<_>>();
        let demand_roots_changed = reset_state || !changed.is_empty() || !removed.is_empty();

        match self.analyze_exchange(
            wire_demands,
            removed.clone(),
            reset_state,
            &reference_locations,
            demand_roots_changed,
        ) {
            Err(SessionError::Service { code, .. })
                if code == "state-mismatch" && self.can_recover() =>
            {
                // The producer lost the state this delta was relative to, so the
                // next request must carry the complete demand set.
                self.clear_retained_state();
                Self::reject_foreign_locations(groups)?;
                let complete = groups
                    .iter()
                    .flat_map(|group| group.demands().iter().cloned())
                    .collect::<Vec<_>>();
                let table =
                    self.analyze_exchange(complete, Vec::new(), true, &reference_locations, true)?;
                self.retain_all_groups(groups);
                Ok(table)
            }
            // A producer that died *after* answering the analyze request, while
            // the symbol phase was still talking to it.
            //
            // `exchange` restarts and replays around one failed request, and
            // that covers the analyze exchange itself. It cannot cover the
            // `Operation::Symbols` exchanges that follow, and those deliberately
            // use `exchange_once`: each carries the `state_token` and the
            // `release_analysis`/`reference_changes` flags of the analysis that
            // just ran, so re-sending one to a freshly spawned producer would
            // name retained state that process never had. Recovery has to redo
            // the analysis, not the request -- which is why it belongs here,
            // where the demand set is still known, rather than inside
            // `close_symbols`.
            //
            // Reaching this arm therefore means nothing has recovered yet: the
            // analyze exchange's own restart either was not entered or already
            // succeeded. So restart, replay the update history, and re-ask for
            // the complete demand set against the new process -- the same shape
            // the `state-mismatch` arm uses, plus the restart it does not need.
            //
            // Exactly once. The re-ask is not wrapped again, so a producer that
            // keeps dying surfaces its failure instead of being retried
            // forever.
            Err(error) if error.is_transport_failure() && self.can_recover() => {
                self.restart_and_replay()?;
                Self::reject_foreign_locations(groups)?;
                let complete = groups
                    .iter()
                    .flat_map(|group| group.demands().iter().cloned())
                    .collect::<Vec<_>>();
                let table =
                    self.analyze_exchange(complete, Vec::new(), true, &reference_locations, true)?;
                self.retain_all_groups(groups);
                Ok(table)
            }
            Ok(table) => {
                self.retain_changed_groups(&changed, &removed);
                Ok(table)
            }
            Err(error) => Err(error),
        }
    }

    fn reject_foreign_locations(groups: &[DemandGroup<'_>]) -> Result<(), SessionError> {
        for group in groups {
            if let Some(foreign) = group.foreign_location() {
                return Err(SessionError::InvalidResponse(format!(
                    "demand group for {} carries a location in {foreign}",
                    group.path()
                )));
            }
        }
        Ok(())
    }

    /// Replaces the retained runs the producer just accepted. Cloning here is
    /// proportional to what changed, not to the whole demand set.
    fn retain_changed_groups(&mut self, changed: &[DemandGroup<'_>], removed: &[String]) {
        for path in removed {
            self.retained_demands.remove(path);
        }
        for group in changed {
            self.retained_demands
                .insert(group.path().to_owned(), group.retained());
        }
    }

    fn retain_all_groups(&mut self, groups: &[DemandGroup<'_>]) {
        self.retained_demands.clear();
        for group in groups {
            self.retained_demands
                .insert(group.path().to_owned(), group.retained());
        }
    }

    /// Sends an update, runs `work`, then waits for the producer to acknowledge
    /// the new generation.
    ///
    /// The caller's work overlaps the acknowledgement, so an edit pays
    /// `max(update, work)` instead of their sum. The scoping is what makes that
    /// safe: `work` cannot touch the session, so no analysis can be sent ahead of
    /// the update it depends on, and the acknowledgement is awaited on every path
    /// out of this call — including when `work` returns an error or panics.
    ///
    /// `work` returns its own value untouched, so a fallible caller can pass a
    /// closure returning `Result` and handle that failure once the session is
    /// back in a consistent state.
    pub fn update_during<I, T>(
        &mut self,
        changes: I,
        work: impl FnOnce() -> T,
    ) -> Result<T, SessionError>
    where
        I: IntoIterator<Item = FileChange>,
    {
        let result = self.update_during_unguarded(changes, work);
        self.guard(result)
    }

    fn update_during_unguarded<I, T>(
        &mut self,
        changes: I,
        work: impl FnOnce() -> T,
    ) -> Result<T, SessionError>
    where
        I: IntoIterator<Item = FileChange>,
    {
        self.ensure_open()?;
        let changes = changes.into_iter().collect::<Vec<_>>();
        if changes.is_empty() {
            return Ok(work());
        }

        let mut sending = request(Operation::Update, &self.project_id, self.generation + 1);
        sending.changes.clone_from(&changes);
        let sent = match self.send_once(sending) {
            Ok(sent) => sent,
            Err(error) if error.is_transport_failure() && self.can_recover() => {
                // The producer died before it could be told. Recover, then fall
                // back to a plain update: nothing is in flight to overlap with.
                self.restart_and_replay()?;
                let worked = work();
                self.update(changes)?;
                return Ok(worked);
            }
            Err(error) => return Err(error),
        };

        // The acknowledgement is collected on every path out of here. A panic in
        // `work` would otherwise unwind past the wait and leave the session one
        // generation behind the producer, so it is caught, the update finished,
        // and the panic resumed.
        let worked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
        let finished = self.finish_update(sent, changes);
        match worked {
            Ok(value) => {
                finished?;
                Ok(value)
            }
            Err(panic) => {
                // The session is consistent again; let the original failure be
                // the one the caller sees.
                drop(finished);
                std::panic::resume_unwind(panic)
            }
        }
    }

    /// Collects an update acknowledgement and advances the generation exactly
    /// once, recovering if the producer died while the request was in flight.
    fn finish_update(
        &mut self,
        sent: SentRequest,
        changes: Vec<FileChange>,
    ) -> Result<(), SessionError> {
        let send = sent.request_send;
        let wait_started = Instant::now();
        let outcome = self
            .connection
            .as_ref()
            .ok_or(SessionError::Closed)?
            .wait(sent);
        let wait = wait_started.elapsed();
        self.last_update_timings = Some(UpdateTimings { send, wait });
        let generation = self.generation + 1;
        let outcome = outcome.and_then(|response| {
            validate_response_identity(&response, &self.project_id, generation).map(|()| response)
        });
        match outcome {
            Ok(response) => {
                self.commit_update(changes, response.affected);
                Ok(())
            }
            Err(error) if error.is_transport_failure() && self.can_recover() => {
                // This update is not in the replay state yet, so the replay
                // restores everything before it and it is re-sent exactly once.
                self.restart_and_replay()?;
                let mut retry = request(Operation::Update, &self.project_id, self.generation + 1);
                retry.changes.clone_from(&changes);
                let response = self.exchange_once(&mut retry)?;
                self.commit_update(changes, response.affected);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Records an acknowledged update: one generation, one replay batch.
    fn commit_update(&mut self, changes: Vec<FileChange>, affected: Vec<String>) {
        self.generation += 1;
        self.affected_paths.extend(affected);
        self.affected_paths
            .extend(changes.iter().map(|change| change.path.clone()));
        self.supersede_replayed_overlays(&changes);
        self.replay_batches.push(changes);
    }

    fn send_once(&self, mut sending: Request) -> Result<SentRequest, SessionError> {
        self.connection
            .as_ref()
            .ok_or(SessionError::Closed)?
            .send(&mut sending)
    }

    /// Sends an update and waits for it, doing nothing in between.
    ///
    /// Equivalent to `update_during(changes, || ())`; kept because most callers
    /// have no work to overlap and should not have to say so.
    pub fn update<I>(&mut self, changes: I) -> Result<(), SessionError>
    where
        I: IntoIterator<Item = FileChange>,
    {
        self.update_during(changes, || ())
    }

    /// Drops the overlays a new batch makes redundant. Only the newest overlay
    /// per path affects a replayed generation, so keeping the older copies would
    /// grow the session by the full source text of every edit it ever sent.
    fn supersede_replayed_overlays(&mut self, changes: &[FileChange]) {
        let next = self.replay_batches.len();
        for change in changes {
            if let Some(batch) = self.replay_index.insert(change.path.clone(), next)
                && let Some(previous) = self.replay_batches.get_mut(batch)
            {
                previous.retain(|kept| kept.path != change.path);
            }
        }
    }

    pub fn configured_sources(&mut self) -> Result<Vec<SourceFile>, SessionError> {
        let result = self.configured_sources_unguarded();
        self.guard(result)
    }

    fn configured_sources_unguarded(&mut self) -> Result<Vec<SourceFile>, SessionError> {
        self.ensure_open()?;
        let response = self.exchange(request(
            Operation::Sources,
            &self.project_id,
            self.generation,
        ))?;
        decode_sources(response)
    }

    /// Reports the resolved module graph of the open generation: the files the
    /// TypeScript program actually included, and — when asked for — where each
    /// module specifier resolved and which package owns it.
    ///
    /// This is a read of the retained program, not of a fact table. It carries
    /// no state token, edits no retained demand set, and leaves any
    /// materialized analysis untouched, so it may be issued at any point in a
    /// session's life without disturbing it.
    ///
    /// A backend that cannot report the graph fails the request rather than
    /// answering a partial one: an inventory that omits files the analysis read,
    /// presented as the complete list, is the defect an attested graph exists to
    /// remove.
    pub fn module_graph(
        &mut self,
        demand: &ModuleGraphDemand,
    ) -> Result<ModuleGraph, SessionError> {
        let result = self.module_graph_unguarded(demand);
        self.guard(result)
    }

    fn module_graph_unguarded(
        &mut self,
        demand: &ModuleGraphDemand,
    ) -> Result<ModuleGraph, SessionError> {
        self.ensure_open()?;
        let mut modules = request(Operation::Modules, &self.project_id, self.generation);
        modules.module_graph = Some(ModuleGraphRequest {
            imports: demand.imports,
            import_paths: demand.import_paths.clone(),
            packages: demand.packages,
        });
        let response = self.exchange(modules)?;
        Ok(ModuleGraph {
            modules: response.modules,
            imports: response.module_imports,
            unknown_import_paths: response.unknown_import_paths,
        })
    }

    /// Closes the session, returning what the producer did about it.
    ///
    /// **A producer that is already gone is not a failure to close it.**
    /// Everything `close` promises has happened before this returns, on every
    /// path: the session is marked closed and the child is terminated whether
    /// or not the goodbye was acknowledged. A transport failure here says the
    /// process died before it could answer -- which is the state `close` was
    /// asking it to reach -- and the 250 ms bound means the same thing, because
    /// the connection is torn down regardless of which wins.
    ///
    /// Reporting that as `Err` said "close failed" about a close that had
    /// succeeded, and it was the only place in this API where a producer death
    /// the session can absorb is an error: everywhere else the restart
    /// machinery recovers one and answers `Ok`. The information is still worth
    /// having, so it comes back as [`CloseOutcome`] rather than as a failure.
    ///
    /// Narrow on purpose: only transport failures become
    /// [`CloseOutcome::ProducerAlreadyGone`]. A service-level refusal is the
    /// producer answering and declining, which is a real answer and still
    /// propagates.
    pub fn close(&mut self) -> Result<CloseOutcome, SessionError> {
        if self.closed {
            return Ok(CloseOutcome::Acknowledged);
        }
        let mut close = request(Operation::Close, &self.project_id, self.generation);
        let result = self
            .connection
            .as_ref()
            .ok_or(SessionError::Closed)
            .and_then(|connection| connection.send(&mut close))
            .and_then(|sent| {
                self.connection
                    .as_ref()
                    .ok_or(SessionError::Closed)?
                    .wait_for_close(sent, Duration::from_millis(250))
            });
        self.closed = true;
        if let Some(mut connection) = self.connection.take() {
            connection.terminate();
        }
        match result {
            Ok(_) => Ok(CloseOutcome::Acknowledged),
            Err(error) if error.is_transport_failure() => Ok(CloseOutcome::ProducerAlreadyGone),
            Err(error) => Err(error),
        }
    }

    fn analyze_exchange(
        &mut self,
        wire_demands: Vec<EntityDemand>,
        removed_demand_paths: Vec<String>,
        reset_state: bool,
        reference_locations: &HashSet<crate::Location>,
        demand_roots_changed: bool,
    ) -> Result<FactTable, SessionError> {
        let (demands, compact_demands) = if reset_state && !wire_demands.is_empty() {
            (Vec::new(), Some(v3::compact_demands(&wire_demands)))
        } else {
            (wire_demands, None)
        };
        let mut analyze = request(Operation::Analyze, &self.project_id, self.generation);
        analyze.demands = demands;
        analyze.compact_demands = compact_demands;
        analyze.state_token = if reset_state {
            String::new()
        } else {
            self.state_token.clone()
        };
        analyze.reset_state = reset_state;
        analyze.removed_demand_paths = removed_demand_paths;
        if !demand_roots_changed && self.retained_table.is_some() {
            let invalidated = self.invalidated_symbol_ids();
            analyze.symbol_queries = invalidated
                .into_iter()
                .map(|id| SymbolQuery {
                    id,
                    references: false,
                    references_only: false,
                })
                .collect();
            analyze.reference_paths = self.affected_paths.iter().cloned().collect();
            analyze.reference_paths.sort();
            analyze.release_analysis = true;
            analyze.reference_changes = true;
        }
        let response = self.exchange(analyze)?;
        self.last_exchange_timings = Some(exchange_timings(&response));
        let (mut candidate, mut changes) = prepare_analyze_response(
            &response,
            &self.project_id,
            self.generation,
            &mut self.retained_table,
            &self.state_token,
        )?;
        if response.timings.is_some_and(|timings| timings.materialized) {
            changes.symbol_ids = self.close_symbols(
                &mut candidate,
                reference_locations,
                demand_roots_changed
                    || !changes.entity_paths.is_empty()
                    || !changes.file_paths.is_empty(),
                &response.state_token,
                Some(&response),
            )?;
            changes.unchanged = changes.entity_paths.is_empty()
                && changes.file_paths.is_empty()
                && changes.symbol_ids.is_empty();
        }
        let returned = candidate.clone();
        // The candidate and successor token publish together. All decoding,
        // identity checks, and candidate construction completed above.
        self.retained_table = Some(candidate);
        self.affected_paths.clear();
        self.state_token = response.state_token;
        self.last_table_changes = Some(changes);
        Ok(returned)
    }

    /// Closes Rust-owned symbol reachability through the one batched TSGo
    /// oracle operation. Rows cross the process seam exactly once per phase
    /// and are retained only in Rust.
    fn close_symbols(
        &mut self,
        table: &mut FactTable,
        reference_locations: &HashSet<crate::Location>,
        seeds_changed: bool,
        state_token: &str,
        prefetched: Option<&Response>,
    ) -> Result<Vec<String>, SessionError> {
        if !seeds_changed
            && table.symbol_count() != 0
            && let Some(changed) = self.patch_stable_symbols(table, state_token, prefetched)?
        {
            return Ok(changed);
        }
        let mut seen = HashSet::<Arc<str>>::with_capacity(table.entity_count());
        let mut pending = Vec::<Arc<str>>::with_capacity(table.entity_count());
        let mut reference_roots = Vec::with_capacity(reference_locations.len());
        {
            let mut enqueue = |id: &Arc<str>| {
                if !id.is_empty() && seen.insert(Arc::clone(id)) {
                    pending.push(Arc::clone(id));
                }
            };
            for entity in table.entities() {
                enqueue(&entity.symbol);
                if let Some(call) = &entity.resolved_call {
                    enqueue(&call.target);
                }
                if reference_locations.contains(&entity.location) && !entity.symbol.is_empty() {
                    reference_roots.push(Arc::clone(&entity.symbol));
                }
            }
            for file in table.files() {
                for function in file.async_functions.iter() {
                    enqueue(&function.symbol);
                    enqueue(&function.target);
                }
            }
        }
        let mut facts = HashMap::<Arc<str>, crate::SymbolFact>::with_capacity(pending.len());
        let mut requested_reference_changes = false;
        let mut reference_changes_exact = false;
        let mut changed_references = HashSet::<Arc<str>>::new();
        while !pending.is_empty() {
            let mut request = request(Operation::Symbols, &self.project_id, self.generation);
            request.state_token = state_token.to_owned();
            request.symbol_queries = pending
                .drain(..)
                .map(|id| SymbolQuery {
                    id,
                    references: false,
                    references_only: false,
                })
                .collect();
            request
                .symbol_queries
                .sort_by(|left, right| left.id.cmp(&right.id));
            request.reference_changes = !requested_reference_changes;
            requested_reference_changes = true;
            let expected = request
                .symbol_queries
                .iter()
                .map(|query| Arc::clone(&query.id))
                .collect::<Vec<_>>();
            let response = self.exchange_once(&mut request)?;
            if request.reference_changes {
                reference_changes_exact = response.reference_changes_exact;
                changed_references.extend(response.changed_reference_symbols.iter().cloned());
            }
            if response.symbol_evidence.len() != expected.len() {
                return Err(SessionError::InvalidResponse(format!(
                    "symbol oracle returned {} rows for {} queries",
                    response.symbol_evidence.len(),
                    expected.len()
                )));
            }
            for (expected_id, fact) in expected.into_iter().zip(response.symbol_evidence) {
                if fact.id != expected_id {
                    return Err(SessionError::InvalidResponse(format!(
                        "symbol oracle returned {:?}, expected {:?}",
                        fact.id, expected_id
                    )));
                }
                if !fact.alias_target.is_empty() && seen.insert(Arc::clone(&fact.alias_target)) {
                    pending.push(Arc::clone(&fact.alias_target));
                }
                facts.insert(expected_id, fact);
            }
        }
        if !requested_reference_changes {
            let mut request = request(Operation::Symbols, &self.project_id, self.generation);
            request.state_token = state_token.to_owned();
            request.reference_changes = true;
            let response = self.exchange_once(&mut request)?;
            reference_changes_exact = response.reference_changes_exact;
            changed_references.extend(response.changed_reference_symbols);
        }

        let mut full = HashSet::<Arc<str>>::with_capacity(reference_roots.len());
        let mut full_queue = Vec::with_capacity(reference_roots.len());
        for id in &reference_roots {
            if !id.is_empty() && full.insert(Arc::clone(id)) {
                full_queue.push(Arc::clone(id));
            }
        }
        let mut full_index = 0;
        while full_index < full_queue.len() {
            let id = &full_queue[full_index];
            if let Some(fact) = facts.get(id)
                && !fact.alias_target.is_empty()
                && full.insert(Arc::clone(&fact.alias_target))
            {
                full_queue.push(Arc::clone(&fact.alias_target));
            }
            full_index += 1;
        }
        let full_ids = full
            .into_iter()
            .filter(|id| {
                facts
                    .get(id)
                    .is_some_and(|fact| fact.alias_target.is_empty())
            })
            .collect::<Vec<_>>();
        let mut reference_ids = full_ids
            .iter()
            .filter(|id| {
                !reference_changes_exact
                    || changed_references.contains(*id)
                    || table.symbol_fact(id).is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        reference_ids.sort();
        let mut released = false;
        if !reference_ids.is_empty() {
            let mut request = request(Operation::Symbols, &self.project_id, self.generation);
            request.state_token = state_token.to_owned();
            request.symbol_queries = reference_ids
                .iter()
                .map(|id| SymbolQuery {
                    id: Arc::clone(id),
                    references: true,
                    references_only: true,
                })
                .collect();
            request.release_analysis = true;
            let response = self.exchange_once(&mut request)?;
            released = true;
            if response.symbol_evidence.len() != reference_ids.len() {
                return Err(SessionError::InvalidResponse(format!(
                    "reference oracle returned {} rows for {} queries",
                    response.symbol_evidence.len(),
                    reference_ids.len()
                )));
            }
            for (expected, fact) in reference_ids.into_iter().zip(response.symbol_evidence) {
                if fact.id != expected
                    || !fact.alias_target.is_empty()
                    || !fact.declarations.is_empty()
                {
                    return Err(SessionError::InvalidResponse(format!(
                        "reference oracle returned invalid row for {expected:?}"
                    )));
                }
                let retained = facts.get_mut(&expected).ok_or_else(|| {
                    SessionError::InvalidResponse(format!(
                        "reference oracle returned unreachable row for {expected:?}"
                    ))
                })?;
                retained.references = fact.references;
            }
        }
        if reference_changes_exact {
            for id in &full_ids {
                if changed_references.contains(id) {
                    continue;
                }
                if let Some(retained) = table.symbol_fact(id)
                    && let Some(fact) = facts.get_mut(id)
                {
                    fact.references = retained.references;
                }
            }
        }
        let mut facts = facts.into_values().collect::<Vec<_>>();
        facts.sort_by(|left, right| left.id.cmp(&right.id));
        if !released {
            let mut release = request(Operation::Symbols, &self.project_id, self.generation);
            release.state_token = state_token.to_owned();
            release.release_analysis = true;
            self.exchange_once(&mut release)?;
        }
        self.reference_tier = full_ids.into_iter().collect();
        let changed = table.replace_symbols(facts);
        self.rebuild_symbol_path_index(table);
        Ok(changed)
    }

    fn patch_stable_symbols(
        &mut self,
        table: &mut FactTable,
        state_token: &str,
        prefetched: Option<&Response>,
    ) -> Result<Option<Vec<String>>, SessionError> {
        let invalidated = self.invalidated_symbol_ids();
        let owned_response;
        let response = if let Some(response) = prefetched {
            response
        } else {
            let mut probe = request(Operation::Symbols, &self.project_id, self.generation);
            probe.state_token = state_token.to_owned();
            probe.reference_changes = true;
            probe.symbol_queries = invalidated
                .iter()
                .map(|id| SymbolQuery {
                    id: Arc::clone(id),
                    references: false,
                    references_only: false,
                })
                .collect();
            owned_response = self.exchange_once(&mut probe)?;
            &owned_response
        };
        if !response.reference_changes_exact {
            return Ok(None);
        }
        if response.symbol_evidence.len() != invalidated.len() {
            return Err(SessionError::InvalidResponse(format!(
                "stable symbol oracle returned {} rows for {} queries",
                response.symbol_evidence.len(),
                invalidated.len()
            )));
        }
        let invalidated_set = invalidated.iter().cloned().collect::<HashSet<_>>();
        let mut patches = HashMap::<Arc<str>, crate::SymbolFact>::new();
        for mut fact in response.symbol_evidence.iter().cloned() {
            let expected = Arc::clone(&fact.id);
            if !invalidated_set.contains(&expected) {
                return Err(SessionError::InvalidResponse(format!(
                    "stable symbol oracle returned unexpected row {:?}",
                    fact.id
                )));
            }
            let retained = table.symbol_fact(&expected).ok_or_else(|| {
                SessionError::InvalidResponse(format!("retained symbol {expected:?} disappeared"))
            })?;
            if fact.alias_target != retained.alias_target {
                return Ok(None);
            }
            fact.references = retained.references;
            patches.insert(expected, fact);
        }
        let mut refresh = response
            .changed_reference_symbols
            .iter()
            .filter(|&id| self.reference_tier.contains(id) && table.symbol(id).is_some())
            .cloned()
            .collect::<Vec<_>>();
        refresh.sort();
        refresh.dedup();
        let mut released = prefetched.is_some();
        let mut reference_patches = Vec::new();
        if !response.reference_evidence.is_empty() {
            let eligible = refresh.iter().cloned().collect::<HashSet<_>>();
            for fact in &response.reference_evidence {
                if eligible.contains(&fact.id) && fact.alias_target.is_empty() {
                    reference_patches.push(fact.clone());
                }
            }
            let covered = reference_patches
                .iter()
                .map(|fact| Arc::clone(&fact.id))
                .collect::<HashSet<_>>();
            refresh.retain(|id| !covered.contains(id));
        }
        if !refresh.is_empty() {
            let mut request = request(Operation::Symbols, &self.project_id, self.generation);
            request.state_token = state_token.to_owned();
            request.symbol_queries = refresh
                .iter()
                .map(|id| SymbolQuery {
                    id: Arc::clone(id),
                    references: true,
                    references_only: false,
                })
                .collect();
            request.release_analysis = true;
            let response = self.exchange_once(&mut request)?;
            released = true;
            if response.symbol_evidence.len() != refresh.len() {
                return Err(SessionError::InvalidResponse(format!(
                    "stable reference oracle returned {} rows for {} queries",
                    response.symbol_evidence.len(),
                    refresh.len()
                )));
            }
            for (expected, fact) in refresh.into_iter().zip(response.symbol_evidence) {
                if fact.id != expected || !fact.alias_target.is_empty() {
                    return Err(SessionError::InvalidResponse(format!(
                        "stable reference oracle returned invalid row for {expected:?}"
                    )));
                }
                patches.insert(expected, fact);
            }
        }
        for id in &invalidated_set {
            if let Some(previous) = table.symbol_fact(id) {
                for declaration in previous.declarations.iter() {
                    let path = declaration.location.path.as_ref();
                    if let Some(ids) = self.symbols_by_path.get_mut(path) {
                        ids.retain(|candidate| candidate != id);
                        if ids.is_empty() {
                            self.symbols_by_path.remove(path);
                        }
                    }
                }
            }
        }
        let structural = patches
            .values()
            .filter(|fact| invalidated_set.contains(&fact.id))
            .cloned()
            .collect::<Vec<_>>();
        let mut changed = table.patch_symbols(patches.into_values().collect());
        let mut reference_paths = self.affected_paths.iter().cloned().collect::<Vec<_>>();
        reference_paths.sort();
        for fact in reference_patches {
            if table.patch_reference_paths(&fact.id, &reference_paths, &fact.references) {
                changed.push(fact.id.to_string());
            }
        }
        changed.sort();
        changed.dedup();
        for fact in structural {
            for declaration in fact.declarations.iter() {
                let ids = self
                    .symbols_by_path
                    .entry(Arc::clone(&declaration.location.path))
                    .or_default();
                if !ids.contains(&fact.id) {
                    ids.push(Arc::clone(&fact.id));
                }
            }
        }
        if !released {
            let mut release = request(Operation::Symbols, &self.project_id, self.generation);
            release.state_token = state_token.to_owned();
            release.release_analysis = true;
            self.exchange_once(&mut release)?;
        }
        Ok(Some(changed))
    }

    fn invalidated_symbol_ids(&self) -> Vec<Arc<str>> {
        self.affected_paths
            .iter()
            .filter_map(|path| self.symbols_by_path.get(path.as_str()))
            .flatten()
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }

    fn rebuild_symbol_path_index(&mut self, table: &FactTable) {
        self.symbols_by_path.clear();
        for symbol in table.symbols() {
            for declaration in symbol.declarations() {
                self.symbols_by_path
                    .entry(Arc::clone(&declaration.location.path))
                    .or_default()
                    .push(Arc::from(symbol.id()));
            }
        }
    }

    fn exchange(&mut self, mut request: Request) -> Result<Response, SessionError> {
        match self.exchange_once(&mut request) {
            Err(error) if error.is_transport_failure() && self.can_recover() => {
                self.restart_and_replay()?;
                self.exchange_once(&mut request)
            }
            result => result,
        }
    }

    fn exchange_once(&self, request: &mut Request) -> Result<Response, SessionError> {
        let operation = request.operation;
        let mut response = self
            .connection
            .as_ref()
            .ok_or(SessionError::Closed)?
            .exchange(request)?;
        validate_response_identity(&response, &self.project_id, request.generation)?;
        if operation == Operation::Symbols
            && response.symbol_evidence.is_empty()
            && !response.table_transition.is_empty()
        {
            let packed = v3::decode_table_transition(&response.table_transition)
                .map_err(SessionError::InvalidResponse)?;
            if packed.mode != TransitionMode::Full
                || packed.project_id.as_ref() != self.project_id
                || packed.target_generation != self.generation
                || !packed.paths.is_empty()
            {
                return Err(SessionError::InvalidResponse(
                    "packed symbol evidence has an invalid table identity".into(),
                ));
            }
            response.symbol_evidence = packed
                .symbols
                .into_iter()
                .map(|operation| match operation {
                    v3::SymbolOp::Replace(fact) => Ok(fact),
                    _ => Err(SessionError::InvalidResponse(
                        "packed symbol evidence contains a delta operation".into(),
                    )),
                })
                .collect::<Result<Vec<_>, _>>()?;
            response.table_transition.clear();
        }
        Ok(response)
    }

    fn restart_and_replay(&mut self) -> Result<(), SessionError> {
        let Some(producer) = self.producer.as_ref() else {
            return Err(SessionError::Process(
                "a bridged producer is never restarted or replayed".into(),
            ));
        };
        let connection = ProcessConnection::spawn(producer, &self.project_id);
        if let Some(mut connection) = self.connection.take() {
            connection.terminate();
        }
        self.connection = Some(Connection::Process(connection?));
        self.restart_epoch = self.restart_epoch.checked_add(1).ok_or_else(|| {
            SessionError::ProducerProvenance("producer restart epoch overflow".into())
        })?;
        self.generation = 1;
        self.clear_retained_state();
        let mut open = request(Operation::Open, &self.project_id, 1);
        self.exchange_once(&mut open)?;
        for changes in self.replay_batches.clone() {
            let mut update = request(Operation::Update, &self.project_id, self.generation + 1);
            update.changes = changes;
            self.exchange_once(&mut update)?;
            self.generation += 1;
        }
        Ok(())
    }

    fn clear_retained_state(&mut self) {
        self.state_token.clear();
        self.retained_demands.clear();
        self.retained_table = None;
        self.affected_paths.clear();
        self.reference_tier.clear();
        self.symbols_by_path.clear();
    }

    fn ensure_open(&self) -> Result<(), SessionError> {
        if self.closed {
            Err(SessionError::Closed)
        } else {
            Ok(())
        }
    }

    /// Whether a failure may be recovered by relaunching and replaying. Only a
    /// process session can; a bridged one has no producer to relaunch.
    fn can_recover(&self) -> bool {
        self.producer.is_some()
    }

    /// A bridged session's first failure of any kind closes it: its program
    /// is released, its retained state dropped, and no later call reuses a
    /// possibly partly applied answer. A process session is unaffected.
    fn guard<T>(&mut self, result: Result<T, SessionError>) -> Result<T, SessionError> {
        if result.is_err() && !self.can_recover() {
            self.closed = true;
            self.clear_retained_state();
            if let Some(mut connection) = self.connection.take() {
                connection.terminate();
            }
        }
        result
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn request(operation: Operation, project_id: &str, generation: u64) -> Request {
    Request {
        schema: v3::TYPE_FACTS_SCHEMA_V1,
        request_id: 0,
        operation,
        project_id: project_id.into(),
        generation,
        changes: Vec::new(),
        demands: Vec::new(),
        compact_demands: None,
        state_token: String::new(),
        reset_state: false,
        removed_demand_paths: Vec::new(),
        symbol_queries: Vec::new(),
        release_analysis: false,
        reference_changes: false,
        reference_paths: Vec::new(),
        cancel_request_id: 0,
        module_graph: None,
    }
}

fn exchange_timings(response: &Response) -> ExchangeTimings {
    let server = response.timings.unwrap_or_default();
    ExchangeTimings {
        roundtrip: Duration::from_nanos(response.client_roundtrip_ns),
        request_send: Duration::from_nanos(response.client_request_send_ns),
        request_bytes: response.client_request_bytes,
        response_decode: Duration::from_nanos(response.client_decode_ns),
        response_bytes: response.client_response_bytes,
        server_request_decode: Duration::from_nanos(server.request_decode_ns),
        server_analyze: Duration::from_nanos(server.analyze_ns),
        server_async: Duration::from_nanos(server.r#async_ns),
        server_demand: Duration::from_nanos(server.demand_ns),
        server_assembly: Duration::from_nanos(server.assembly_ns),
        server_sort: Duration::from_nanos(server.sort_ns),
        server_close_symbols: Duration::from_nanos(server.close_symbols_ns),
        server_materialized: server.materialized,
        server_retained_files: server.retained_files,
        server_recomputed_files: server.recomputed_files,
        server_non_durable_files: server.non_durable_files,
    }
}

fn prepare_analyze_response(
    response: &Response,
    expected_project: &str,
    expected_generation: u64,
    retained: &mut Option<FactTable>,
    retained_state_token: &str,
) -> Result<(FactTable, TableChanges), SessionError> {
    if response.schema != v3::TYPE_FACTS_SCHEMA_V1 {
        return Err(SessionError::InvalidResponse(format!(
            "response schema is {}, expected {}",
            response.schema,
            v3::TYPE_FACTS_SCHEMA_V1
        )));
    }
    if response.project_id != expected_project || response.generation != expected_generation {
        return Err(SessionError::InvalidResponse(format!(
            "response identity is project {:?} generation {}, expected {:?} generation {}",
            response.project_id, response.generation, expected_project, expected_generation
        )));
    }
    if response.state_token.is_empty() {
        return Err(SessionError::InvalidResponse(
            "retained response has no state token".into(),
        ));
    }

    if response.table_transition.is_empty() {
        let candidate = retained.as_ref().ok_or_else(|| {
            SessionError::InvalidResponse("reuse response has no retained table".into())
        })?;
        if candidate.schema() != v3::TYPE_FACTS_TABLE_SCHEMA_V22
            || candidate.project_id() != expected_project
            || candidate.generation() != response.generation
        {
            return Err(SessionError::InvalidResponse(
                "retained Wire table identity is invalid".into(),
            ));
        }
        let candidate = candidate.clone();
        return Ok((
            candidate,
            TableChanges {
                unchanged: true,
                ..TableChanges::default()
            },
        ));
    }

    let transition = v3::decode_table_transition(&response.table_transition)
        .map_err(SessionError::InvalidResponse)?;
    if transition.project_id.as_ref() != response.project_id.as_str()
        || transition.target_generation != response.generation
        || transition.table_schema != v3::TYPE_FACTS_TABLE_SCHEMA_V22
    {
        return Err(SessionError::InvalidResponse(
            "table transition identity does not match response".into(),
        ));
    }
    match transition.mode {
        TransitionMode::Full => Ok(materialize_full_transition(transition)),
        TransitionMode::Delta => {
            let retained_table = retained.as_ref().ok_or_else(|| {
                SessionError::InvalidResponse("delta transition has no retained table".into())
            })?;
            if retained_table.schema() != transition.table_schema
                || retained_table.generation() != transition.base_generation
                || retained_table.project_id() != transition.project_id.as_ref()
                || retained_state_token != transition.base_state_token.as_ref()
            {
                return Err(SessionError::InvalidResponse(
                    "delta transition base identity does not match retained state".into(),
                ));
            }
            validate_table_transition_application(retained_table, &transition)?;
            let changes = table_changes_against(retained_table, &transition);
            let candidate = retained_table.apply_delta(transition);
            Ok((candidate, changes))
        }
    }
}

fn table_changes(transition: &WireTableTransition) -> TableChanges {
    let mut entity_paths = Vec::new();
    let mut file_paths = Vec::new();
    for path in &transition.paths {
        if !matches!(&path.entities, SlotOp::Unchanged) {
            entity_paths.push(path.path.to_string());
        }
        if !matches!(&path.file, SlotOp::Unchanged) {
            file_paths.push(path.path.to_string());
        }
    }
    let mut symbol_ids = transition
        .symbols
        .iter()
        .map(|operation| operation.id().to_string())
        .collect::<Vec<_>>();
    symbol_ids.dedup();
    let unchanged = transition.mode == TransitionMode::Delta
        && entity_paths.is_empty()
        && symbol_ids.is_empty()
        && file_paths.is_empty();
    TableChanges {
        unchanged,
        entity_paths,
        symbol_ids,
        file_paths,
    }
}

// Active V1 path replacements are intentionally unconditional: Rust owns the base
// rows and Go no longer retains them merely to decide equality. Resolve the
// sparse transport candidates against the canonical retained table here so a
// source-only edit does not invalidate downstream entity/file consumers.
fn table_changes_against(table: &FactTable, transition: &WireTableTransition) -> TableChanges {
    let mut entity_paths = Vec::new();
    let mut file_paths = Vec::new();
    for path in &transition.paths {
        let entities_changed = match &path.entities {
            SlotOp::Unchanged => false,
            SlotOp::Replace(entities) => table.entities_for_path(&path.path) != entities,
            SlotOp::Remove => !table.entities_for_path(&path.path).is_empty(),
        };
        if entities_changed {
            entity_paths.push(path.path.to_string());
        }
        let file_changed = match &path.file {
            SlotOp::Unchanged => false,
            SlotOp::Replace(file) => table.file(&path.path) != Some(file),
            SlotOp::Remove => table.file(&path.path).is_some(),
        };
        if file_changed {
            file_paths.push(path.path.to_string());
        }
    }
    let mut symbol_ids = transition
        .symbols
        .iter()
        .map(|operation| operation.id().to_string())
        .collect::<Vec<_>>();
    symbol_ids.dedup();
    let unchanged = entity_paths.is_empty() && symbol_ids.is_empty() && file_paths.is_empty();
    TableChanges {
        unchanged,
        entity_paths,
        symbol_ids,
        file_paths,
    }
}

/// Splits a flat demand list into per-path runs, in first-seen path order.
///
/// Only the flat compatibility path needs this. It is the clone that
/// `analyze_groups` exists to avoid.
fn group_demands(demands: &[EntityDemand]) -> Vec<Vec<EntityDemand>> {
    let mut order: Vec<Vec<EntityDemand>> = Vec::new();
    let mut runs: HashMap<&str, usize> = HashMap::new();
    for demand in demands {
        let path: &str = &demand.location.path;
        match runs.get(path) {
            Some(&index) => order[index].push(demand.clone()),
            None => {
                runs.insert(path, order.len());
                order.push(vec![demand.clone()]);
            }
        }
    }
    order
}

// Checks the only table-relative invariant the packed decoder cannot know:
// reference-path operations must name a non-alias symbol that exists after
// preceding operations for the same canonical symbol ID.
fn validate_table_transition_application(
    table: &FactTable,
    transition: &WireTableTransition,
) -> Result<(), SessionError> {
    table
        .validate_delta(transition)
        .map_err(SessionError::InvalidResponse)
}

/// Validates and applies one fully decoded transition to a private test
/// candidate. Session uses the split validation/application calls so it can
/// take unique ownership only after every fallible check succeeds.
#[cfg(test)]
fn apply_table_transition(
    table: &mut FactTable,
    transition: WireTableTransition,
) -> Result<TableChanges, SessionError> {
    validate_table_transition_application(table, &transition)?;
    match transition.mode {
        TransitionMode::Full => {
            let (replacement, changes) = materialize_full_transition(transition);
            *table = replacement;
            Ok(changes)
        }
        TransitionMode::Delta => {
            let changes = table_changes(&transition);
            *table = table.apply_delta(transition);
            Ok(changes)
        }
    }
}

fn materialize_full_transition(transition: WireTableTransition) -> (FactTable, TableChanges) {
    let changes = table_changes(&transition);
    (FactTable::materialize_full(transition), changes)
}

fn decode_sources(response: Response) -> Result<Vec<SourceFile>, SessionError> {
    if response.source_arena.is_empty() {
        return Ok(response.sources);
    }
    if response.source_lengths.len() != response.sources.len() {
        return Err(SessionError::InvalidResponse(
            "source arena descriptor count mismatch".into(),
        ));
    }
    let arena_path = response.source_arena;
    let decoded = (|| {
        let file = std::fs::File::open(&arena_path)
            .map_err(|error| SessionError::Process(format!("open source arena: {error}")))?;
        let mut reader = BufReader::with_capacity(1 << 20, file);
        let mut sources = Vec::with_capacity(response.sources.len());
        for (mut source, length) in response.sources.into_iter().zip(response.source_lengths) {
            let length = usize::try_from(length).map_err(|_| {
                SessionError::InvalidResponse("source arena length overflow".into())
            })?;
            source.source.resize(length, 0);
            reader
                .read_exact(&mut source.source)
                .map_err(|error| SessionError::Process(format!("read source arena: {error}")))?;
            source.local = false;
            sources.push(source);
        }
        let mut trailing = [0_u8; 1];
        if reader
            .read(&mut trailing)
            .map_err(|error| SessionError::Process(format!("finish source arena: {error}")))?
            != 0
        {
            return Err(SessionError::InvalidResponse(
                "source arena has trailing bytes".into(),
            ));
        }
        Ok(sources)
    })();
    let _ = std::fs::remove_file(arena_path);
    decoded
}

fn fail_pending(pending: &PendingResponses, message: String) {
    if let Ok(mut pending) = pending.lock() {
        for (_, sender) in pending.drain() {
            let _ = sender.send(Err(message.clone()));
        }
    }
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests;
