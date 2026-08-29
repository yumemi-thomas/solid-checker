use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs::File,
    io::{BufReader, BufWriter, Read},
    path::{Component, Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};
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

static SESSION_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
struct CertificationProducerPin {
    executable_sha256: crate::SourceHash,
    source_manifest_sha256: crate::SourceHash,
}

/// An explicitly located Type Facts producer.
///
/// This type intentionally performs no environment or executable-relative
/// lookup. Packaging is a consumer concern.
#[derive(Clone, Debug)]
pub struct Producer {
    path: PathBuf,
    args: Vec<OsString>,
    shared_transition_arena: bool,
    certification_pin: Option<CertificationProducerPin>,
}

impl Producer {
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            args: Vec::new(),
            shared_transition_arena: true,
            certification_pin: None,
        }
    }

    /// Locates an execution image whose bytes and source manifest were pinned
    /// by trusted certification configuration.
    ///
    /// The backend is responsible for putting the image in a private execution
    /// snapshot before calling this constructor. The client hashes that exact
    /// path again immediately before every launch, including restart, so a
    /// changed image cannot inherit the pin or live-session authority.
    pub fn pinned_for_certification(
        path: impl Into<PathBuf>,
        executable_sha256: impl Into<String>,
        source_manifest_sha256: impl Into<String>,
    ) -> Result<Self, SessionError> {
        let path = path.into();
        let executable_sha256 = crate::SourceHash::parse(executable_sha256.into())?;
        let source_manifest_sha256 = crate::SourceHash::parse(source_manifest_sha256.into())?;
        let actual = sha256_file(&path)?;
        if actual != executable_sha256 {
            return Err(SessionError::ProducerProvenance(
                "execution image digest does not match its certification pin".into(),
            ));
        }
        Ok(Self {
            path,
            args: Vec::new(),
            shared_transition_arena: true,
            certification_pin: Some(CertificationProducerPin {
                executable_sha256,
                source_manifest_sha256,
            }),
        })
    }

    /// Uses the legacy inline transition payload instead of the default
    /// Rust-owned shared arena.
    #[must_use]
    pub fn without_shared_transition_arena(mut self) -> Self {
        self.shared_transition_arena = false;
        self.certification_pin = None;
        self
    }

    /// Adds producer-specific arguments before the crate-owned `-project`
    /// argument. This is primarily useful for producer diagnostics.
    #[must_use]
    pub fn with_arg(mut self, argument: impl Into<OsString>) -> Self {
        self.args.push(argument.into());
        // Certification launches one exact, policy-owned command line. A
        // caller-added shim, preload, or diagnostic flag is a different
        // runtime image and cannot inherit the executable pin.
        self.certification_pin = None;
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

struct Connection {
    child: Child,
    executable_sha256: crate::SourceHash,
    handshake: Handshake,
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

impl Connection {
    fn spawn(producer: &Producer, project_id: &str) -> Result<Self, SessionError> {
        let executable_sha256 = sha256_file(&producer.path)?;
        if let Some(pin) = &producer.certification_pin
            && executable_sha256 != pin.executable_sha256
        {
            return Err(SessionError::ProducerProvenance(
                "execution image changed before launch".into(),
            ));
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
        let (handshake, mut output) = match handshake_receiver.recv_timeout(Duration::from_secs(5))
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
                    "producer did not report compatibility within 5 seconds".into(),
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
        if actual != expected {
            terminate_child(&mut child);
            return Err(SessionError::Handshake(format!(
                "expected protocol {}, schema {}, build {:?}; got protocol {}, schema {}, build {:?}",
                expected.0, expected.1, expected.2, actual.0, actual.1, actual.2
            )));
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
                let decode_started = Instant::now();
                let response = match decode_trusted::<Response>(&payload) {
                    Ok(mut response) => {
                        response.client_response_bytes =
                            u64::try_from(payload.len()).unwrap_or(u64::MAX);
                        if let Some(arena) = &reader_arena
                            && let Err(error) = arena.attach(&mut response)
                        {
                            fail_pending(&reader_pending, error.to_string());
                            break;
                        }
                        response.client_decode_ns =
                            u64::try_from(decode_started.elapsed().as_nanos()).unwrap_or(u64::MAX);
                        response
                    }
                    Err(error) => {
                        fail_pending(&reader_pending, error.to_string());
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
            executable_sha256,
            handshake,
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
        let SentRequest {
            request_id,
            receiver,
            sent_at,
            request_send,
            request_bytes,
            cancellable,
        } = sent;
        let response = receiver
            .recv()
            .map_err(|_| SessionError::Process("producer response channel closed".into()))?
            .map_err(SessionError::Process);
        if cancellable {
            self.clear_active_request(request_id);
        }
        let mut response = response?;
        response.client_roundtrip_ns =
            u64::try_from(sent_at.elapsed().as_nanos()).unwrap_or(u64::MAX);
        response.client_request_send_ns =
            u64::try_from(request_send.as_nanos()).unwrap_or(u64::MAX);
        response.client_request_bytes = request_bytes;
        if response.request_id != request_id {
            return Err(SessionError::InvalidResponse(
                "request identity mismatch".into(),
            ));
        }
        if !response.ok {
            let error = response.error.ok_or_else(|| {
                SessionError::InvalidResponse("error response has no body".into())
            })?;
            return Err(SessionError::Service {
                code: error.code,
                message: error.message,
            });
        }
        Ok(response)
    }

    fn exchange(&self, request: &mut Request) -> Result<Response, SessionError> {
        let sent = self.send(request)?;
        self.wait(sent)
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

impl Drop for Connection {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// A retained Type Facts session.
///
/// Framing, request identities, handshake validation, Wire table transitions,
/// and subprocess recovery are private implementation details.
pub struct Session {
    producer: Producer,
    project_id: String,
    generation: u64,
    session_id: crate::SourceHash,
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
        let connection = Connection::spawn(&producer, &project_id)?;
        let session_id = new_session_id(&project_id, &connection);
        let mut session = Self {
            producer,
            project_id,
            generation: 1,
            session_id,
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
        };
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

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn cancellation_handle(&self) -> Option<Cancellation> {
        self.connection
            .as_ref()
            .map(|connection| connection.cancellation_handle(self.project_id.clone()))
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
            Err(SessionError::Service { code, .. }) if code == "state-mismatch" => {
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
        self.ensure_open()?;
        let changes = changes.into_iter().collect::<Vec<_>>();
        if changes.is_empty() {
            return Ok(work());
        }

        let mut sending = request(Operation::Update, &self.project_id, self.generation + 1);
        sending.changes.clone_from(&changes);
        let sent = match self.send_once(sending) {
            Ok(sent) => sent,
            Err(error) if error.is_transport_failure() => {
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
        match outcome {
            Ok(response) => {
                self.commit_update(changes, response.affected);
                Ok(())
            }
            Err(error) if error.is_transport_failure() => {
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

    /// Returns exact selected-signature, actual-to-formal, callable-path,
    /// finite-domain and optional implementation-census facts for one live
    /// generation. The operation is read-only and leaves retained analysis
    /// state untouched.
    pub fn invocations(
        &mut self,
        demands: &[crate::InvocationDemand],
    ) -> Result<crate::InvocationAnswer, SessionError> {
        self.ensure_open()?;
        if demands
            .iter()
            .any(|demand| demand.callable_depth > crate::MAX_INVOCATION_CALLABLE_DEPTH)
        {
            return Err(SessionError::InvalidResponse(format!(
                "invocation callable depth exceeds {}",
                crate::MAX_INVOCATION_CALLABLE_DEPTH
            )));
        }
        let mut request = request(Operation::Invocations, &self.project_id, self.generation);
        request.invocation_demands = demands.to_vec();
        let response = self.exchange(request)?;
        let envelope = response.invocation_envelope.ok_or_else(|| {
            SessionError::InvalidResponse("invocation response has no identity envelope".into())
        })?;
        if response.invocation_transcripts.len() != demands.len() {
            return Err(SessionError::InvalidResponse(format!(
                "invocation response has {} transcripts for {} demands",
                response.invocation_transcripts.len(),
                demands.len()
            )));
        }
        for (index, (transcript, demand)) in response
            .invocation_transcripts
            .iter()
            .zip(demands)
            .enumerate()
        {
            if transcript.location != demand.location {
                return Err(SessionError::InvalidResponse(format!(
                    "invocation transcript {index} location does not match its demand"
                )));
            }
            validate_invocation_transcript(transcript)?;
        }
        validate_invocation_envelope(&envelope, &self.project_id, self.generation, demands)?;
        Ok(crate::InvocationAnswer {
            transcripts: response.invocation_transcripts,
            envelope,
        })
    }

    /// Acquires a certification answer from the exact pinned process owned by
    /// this live session.
    ///
    /// Unlike [`Self::invocations`], this refuses an ordinary unpinned
    /// producer and returns a non-serializable authority token that binds the
    /// response to the process/restart epoch, executable/source-manifest pin,
    /// project generation, verifier-derived demand graph, and snapshot.
    pub fn certification_invocations(
        &mut self,
        context: crate::CertificationInvocationContext,
        demands: &[crate::InvocationDemand],
    ) -> Result<crate::LiveInvocationAnswer, SessionError> {
        let pin = self.producer.certification_pin.clone().ok_or_else(|| {
            SessionError::ProducerProvenance(
                "ordinary producer sessions cannot issue certification evidence".into(),
            )
        })?;
        let answer = self.invocations(demands)?;
        let connection = self.connection.as_ref().ok_or(SessionError::Closed)?;
        let demand_sha256 = crate::SourceHash::parse(answer.envelope.demand_sha256.to_string())?;
        let schema_sha256 = crate::SourceHash::parse(connection.handshake.schema_hash.clone())?;
        let evidence_root = live_invocation_evidence_root(
            &self.session_id,
            self.restart_epoch,
            connection,
            &pin,
            &self.project_id,
            self.generation,
            &demand_sha256,
            &context,
        );
        Ok(crate::LiveInvocationAnswer {
            answer,
            identity: crate::LiveProducerSessionIdentity {
                session_id: self.session_id.clone(),
                restart_epoch: self.restart_epoch,
                process_id: connection.child.id(),
                executable_sha256: connection.executable_sha256.clone(),
                source_manifest_sha256: pin.source_manifest_sha256,
                handshake_protocol: connection.handshake.protocol,
                handshake_schema_sha256: schema_sha256,
                handshake_build: connection.handshake.build_id.clone().into(),
                project_id: self.project_id.clone().into(),
                generation: self.generation,
                demand_sha256,
                context,
                evidence_root,
            },
        })
    }

    pub fn close(&mut self) -> Result<(), SessionError> {
        if self.closed {
            return Ok(());
        }
        let mut close = request(Operation::Close, &self.project_id, self.generation);
        let result = self.exchange_once(&mut close);
        self.closed = true;
        if let Some(mut connection) = self.connection.take() {
            connection.terminate();
        }
        result.map(|_| ())
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
            Err(error) if error.is_transport_failure() => {
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
        if let Some(mut connection) = self.connection.take() {
            connection.terminate();
        }
        let connection = Connection::spawn(&self.producer, &self.project_id)?;
        self.connection = Some(connection);
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
}

fn sha256_file(path: &Path) -> Result<crate::SourceHash, SessionError> {
    let mut file = File::open(path).map_err(|error| {
        SessionError::ProducerProvenance(format!("could not open execution image: {error}"))
    })?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            SessionError::ProducerProvenance(format!("could not hash execution image: {error}"))
        })?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    crate::SourceHash::parse(format!("sha256:{:x}", hash.finalize())).map_err(Into::into)
}

fn new_session_id(project_id: &str, connection: &Connection) -> crate::SourceHash {
    let mut hash = Sha256::new();
    hash_invocation_field(&mut hash, "solid-checker:typefacts:live-session:v1");
    hash_invocation_field(&mut hash, project_id);
    hash_invocation_field(&mut hash, connection.executable_sha256.as_str());
    hash_invocation_field(&mut hash, &connection.child.id().to_string());
    hash_invocation_field(
        &mut hash,
        &SESSION_ID_COUNTER
            .fetch_add(1, Ordering::Relaxed)
            .to_string(),
    );
    hash_invocation_field(
        &mut hash,
        &SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_string(),
    );
    crate::SourceHash::parse(format!("sha256:{:x}", hash.finalize()))
        .expect("a SHA-256 rendering is canonical")
}

#[allow(clippy::too_many_arguments)]
fn live_invocation_evidence_root(
    session_id: &crate::SourceHash,
    restart_epoch: u64,
    connection: &Connection,
    pin: &CertificationProducerPin,
    project_id: &str,
    generation: u64,
    demand_sha256: &crate::SourceHash,
    context: &crate::CertificationInvocationContext,
) -> crate::SourceHash {
    let mut hash = Sha256::new();
    hash_invocation_field(
        &mut hash,
        "solid-checker:typefacts:live-certification-evidence:v1",
    );
    for value in [
        session_id.as_str(),
        connection.executable_sha256.as_str(),
        pin.source_manifest_sha256.as_str(),
        &connection.handshake.protocol.to_string(),
        &connection.handshake.schema_hash,
        &connection.handshake.build_id,
        project_id,
        &generation.to_string(),
        &restart_epoch.to_string(),
        &connection.child.id().to_string(),
        demand_sha256.as_str(),
        context.snapshot_root(),
        context.demand_graph_root(),
    ] {
        hash_invocation_field(&mut hash, value);
    }
    for demand_id in context.proof_demand_ids() {
        hash_invocation_field(&mut hash, demand_id);
    }
    crate::SourceHash::parse(format!("sha256:{:x}", hash.finalize()))
        .expect("a SHA-256 rendering is canonical")
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
        invocation_demands: Vec::new(),
    }
}

fn validate_invocation_envelope(
    envelope: &crate::InvocationEnvelope,
    project_id: &str,
    generation: u64,
    demands: &[crate::InvocationDemand],
) -> Result<(), SessionError> {
    if &*envelope.project_id != project_id || envelope.generation != generation {
        return Err(SessionError::InvalidResponse(
            "invocation envelope project or generation mismatch".into(),
        ));
    }
    if &*envelope.schema_sha256 != v3::TYPE_FACTS_SCHEMA_SHA256
        || &*envelope.producer_build != v3::TYPE_FACTS_BUILD_ID
    {
        return Err(SessionError::InvalidResponse(
            "invocation envelope does not match the live handshake".into(),
        ));
    }
    for (name, digest) in [
        ("demand", &*envelope.demand_sha256),
        ("module graph", &*envelope.module_graph_sha256),
        ("schema", &*envelope.schema_sha256),
    ] {
        crate::SourceHash::parse(digest.to_owned()).map_err(|_| {
            SessionError::InvalidResponse(format!("invocation {name} digest is invalid"))
        })?;
    }
    if envelope.demand_sha256.as_ref() != invocation_demand_digest(demands) {
        return Err(SessionError::InvalidResponse(
            "invocation envelope demand digest does not match the request".into(),
        ));
    }
    let mut previous = None;
    for source in &envelope.sources {
        crate::SourceHash::parse(source.sha256.to_string()).map_err(|_| {
            SessionError::InvalidResponse("invocation source digest is invalid".into())
        })?;
        if previous.is_some_and(|path: &str| path >= &*source.path) {
            return Err(SessionError::InvalidResponse(
                "invocation source digests are not uniquely sorted".into(),
            ));
        }
        previous = Some(&*source.path);
    }
    Ok(())
}

fn validate_invocation_transcript(
    transcript: &crate::InvocationTranscript,
) -> Result<(), SessionError> {
    use crate::{ArgumentBindingDisposition, InvocationDomain};

    let mut complete = HashSet::new();
    if transcript
        .completeness
        .0
        .iter()
        .any(|domain| !complete.insert(*domain))
    {
        return Err(SessionError::InvalidResponse(
            "invocation completeness contains a duplicate domain".into(),
        ));
    }

    if let Some(signature) = &transcript.selected_signature {
        if transcript.kind == crate::CallKind::Unknown
            || !transcript
                .completeness
                .contains(InvocationDomain::Signature)
            || !transcript
                .completeness
                .contains(InvocationDomain::Parameters)
            || !transcript.completeness.contains(InvocationDomain::Result)
        {
            return Err(SessionError::InvalidResponse(
                "selected signature lacks locally complete signature/value domains".into(),
            ));
        }
        crate::SourceHash::parse(signature.identity.to_string()).map_err(|_| {
            SessionError::InvalidResponse("selected signature identity is invalid".into())
        })?;
        if signature.overload_count == 0 || signature.overload_ordinal >= signature.overload_count {
            return Err(SessionError::InvalidResponse(
                "selected signature overload identity is outside its exact census".into(),
            ));
        }
        for (index, parameter) in signature.parameters.iter().enumerate() {
            if parameter.index != index
                || (parameter.rest && index + 1 != signature.parameters.len())
            {
                return Err(SessionError::InvalidResponse(
                    "selected signature formal indices or rest position are invalid".into(),
                ));
            }
            validate_invocation_value(&parameter.value)?;
            validate_callable_paths(
                &parameter.callable_paths,
                parameter.value.alternatives.len(),
            )?;
        }
        validate_invocation_value(&signature.result)?;
        validate_callable_paths(
            &signature.result_callable_paths,
            signature.result.alternatives.len(),
        )?;
        if signature.identity.as_ref() != selected_signature_digest(transcript.kind, signature) {
            return Err(SessionError::InvalidResponse(
                "selected signature identity does not match its inputs".into(),
            ));
        }
    } else if transcript
        .completeness
        .contains(InvocationDomain::Signature)
        || transcript
            .completeness
            .contains(InvocationDomain::Parameters)
        || transcript.completeness.contains(InvocationDomain::Result)
    {
        return Err(SessionError::InvalidResponse(
            "invocation closes signature/value domains without a selected signature".into(),
        ));
    }
    let bindings_complete = transcript.completeness.contains(InvocationDomain::Bindings);
    let mut next_expanded = 0;
    let mut bound_parameters = HashSet::new();
    for (argument_index, binding) in transcript.bindings.iter().enumerate() {
        if binding.argument_index != argument_index {
            return Err(SessionError::InvalidResponse(
                "invocation argument bindings are not ordered by written argument".into(),
            ));
        }
        match binding.disposition {
            ArgumentBindingDisposition::Direct if binding.slots.len() != 1 => {
                return Err(SessionError::InvalidResponse(
                    "direct invocation binding must contain one slot".into(),
                ));
            }
            ArgumentBindingDisposition::ExactTupleSpread if binding.slots.is_empty() => {
                return Err(SessionError::InvalidResponse(
                    "exact tuple spread must contain expanded slots".into(),
                ));
            }
            ArgumentBindingDisposition::UnknownLengthSpread
                if !binding.slots.is_empty() || binding.possible.is_none() =>
            {
                return Err(SessionError::InvalidResponse(
                    "unknown spread must carry only a possible formal range".into(),
                ));
            }
            ArgumentBindingDisposition::Unmapped if !binding.slots.is_empty() => {
                return Err(SessionError::InvalidResponse(
                    "unmapped argument carries exact slots".into(),
                ));
            }
            _ => {}
        }
        if bindings_complete
            && matches!(
                binding.disposition,
                ArgumentBindingDisposition::UnknownLengthSpread
                    | ArgumentBindingDisposition::Unmapped
            )
        {
            return Err(SessionError::InvalidResponse(
                "complete invocation bindings contain an open mapping".into(),
            ));
        }
        if let Some(range) = &binding.possible
            && (range.end_exclusive.is_some_and(|end| end < range.start)
                || (range.unbounded && range.end_exclusive.is_some()))
        {
            return Err(SessionError::InvalidResponse(
                "invocation possible formal range is contradictory".into(),
            ));
        }
        let mut next_tuple = 0;
        if bindings_complete {
            for slot in &binding.slots {
                if slot.expanded_index != next_expanded {
                    return Err(SessionError::InvalidResponse(
                        "complete invocation bindings are not contiguous".into(),
                    ));
                }
                next_expanded += 1;
            }
        }
        for slot in &binding.slots {
            let Some(signature) = transcript.selected_signature.as_ref() else {
                return Err(SessionError::InvalidResponse(
                    "invocation binding exists without a selected signature".into(),
                ));
            };
            let Some(parameter) = signature.parameters.get(slot.parameter_index) else {
                return Err(SessionError::InvalidResponse(
                    "invocation binding names a missing formal parameter".into(),
                ));
            };
            if slot.rest != parameter.rest {
                return Err(SessionError::InvalidResponse(
                    "invocation binding rest marker disagrees with its formal".into(),
                ));
            }
            match binding.disposition {
                ArgumentBindingDisposition::Direct if slot.tuple_index.is_some() => {
                    return Err(SessionError::InvalidResponse(
                        "direct invocation binding carries a tuple index".into(),
                    ));
                }
                ArgumentBindingDisposition::ExactTupleSpread => {
                    if slot.tuple_index != Some(next_tuple) {
                        return Err(SessionError::InvalidResponse(
                            "tuple spread slots are not contiguous".into(),
                        ));
                    }
                    next_tuple += 1;
                }
                _ => {}
            }
            bound_parameters.insert(slot.parameter_index);
        }
    }
    if transcript
        .completeness
        .contains(InvocationDomain::Omissions)
        && !bindings_complete
    {
        return Err(SessionError::InvalidResponse(
            "omissions are closed while bindings are open".into(),
        ));
    }
    let mut omitted = HashSet::new();
    for &index in &transcript.omitted_parameters {
        let Some(parameter) = transcript
            .selected_signature
            .as_ref()
            .and_then(|signature| signature.parameters.get(index))
        else {
            return Err(SessionError::InvalidResponse(
                "omitted invocation parameter does not exist".into(),
            ));
        };
        if !omitted.insert(index)
            || bound_parameters.contains(&index)
            || parameter.rest
            || (!parameter.optional && !parameter.defaulted)
        {
            return Err(SessionError::InvalidResponse(
                "omitted invocation parameter is contradictory".into(),
            ));
        }
    }
    let parameter_count = transcript
        .selected_signature
        .as_ref()
        .map_or(0, |signature| signature.parameters.len());
    if transcript
        .parameter_uses
        .iter()
        .any(|usage| usage.parameter_index >= parameter_count)
    {
        return Err(SessionError::InvalidResponse(
            "parameter-use census names a missing formal".into(),
        ));
    }
    if transcript
        .completeness
        .contains(InvocationDomain::ControlFlow)
        && transcript
            .control_flow
            .as_ref()
            .is_none_or(|flow| !flow.unsupported.is_empty())
    {
        return Err(SessionError::InvalidResponse(
            "control-flow census is closed while absent or unsupported".into(),
        ));
    }
    Ok(())
}

fn validate_invocation_value(value: &crate::InvocationValueFact) -> Result<(), SessionError> {
    for (index, alternative) in value.alternatives.iter().enumerate() {
        if alternative.index != index {
            return Err(SessionError::InvalidResponse(
                "invocation value alternatives are not contiguous".into(),
            ));
        }
    }
    for partition in &value.partitions {
        if partition.complete && partition.cases.is_empty() {
            return Err(SessionError::InvalidResponse(
                "complete finite partition is empty".into(),
            ));
        }
    }
    Ok(())
}

fn validate_callable_paths(
    paths: &[crate::CallablePathFact],
    alternative_count: usize,
) -> Result<(), SessionError> {
    use crate::{Callability, InvocationConstructability, PathPresence, PathSegmentKind};

    let mut seen = HashSet::new();
    for fact in paths {
        if fact.alternative >= alternative_count {
            return Err(SessionError::InvalidResponse(
                "callable path names a missing value alternative".into(),
            ));
        }
        let mut key = format!("{}:", fact.alternative);
        for segment in &fact.path {
            match segment.kind {
                PathSegmentKind::Property
                    if segment.property.is_empty() || segment.index.is_some() =>
                {
                    return Err(SessionError::InvalidResponse(
                        "property path segment is malformed".into(),
                    ));
                }
                PathSegmentKind::Tuple
                    if !segment.property.is_empty() || segment.index.is_none() =>
                {
                    return Err(SessionError::InvalidResponse(
                        "tuple path segment is malformed".into(),
                    ));
                }
                _ => {}
            }
            key.push_str(&format!(
                "{:?}:{}:{:?}/",
                segment.kind, segment.property, segment.index
            ));
        }
        if !seen.insert(key) {
            return Err(SessionError::InvalidResponse(
                "callable paths contain a duplicate alternative/path".into(),
            ));
        }
        if fact.presence == PathPresence::Absent
            && (fact.callability != Callability::Unknown
                || fact.constructability != InvocationConstructability::Unknown
                || fact.declaration.is_some()
                || !fact.complete)
        {
            return Err(SessionError::InvalidResponse(
                "absent callable path carries a positive or open fact".into(),
            ));
        }
    }
    Ok(())
}

fn invocation_demand_digest(demands: &[crate::InvocationDemand]) -> String {
    let mut hasher = Sha256::new();
    hash_invocation_field(&mut hasher, "solid-checker:typefacts:invocations:v1");
    for demand in demands {
        hash_invocation_field(&mut hasher, &demand.location.path);
        hash_invocation_field(&mut hasher, &demand.location.start_byte.to_string());
        hash_invocation_field(&mut hasher, &demand.location.end_byte.to_string());
        hash_invocation_field(&mut hasher, &demand.callable_depth.to_string());
        hash_invocation_field(&mut hasher, if demand.census { "true" } else { "false" });
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn selected_signature_digest(
    kind: crate::CallKind,
    signature: &crate::SelectedSignature,
) -> String {
    let mut hasher = Sha256::new();
    hash_invocation_field(&mut hasher, "solid-checker:typefacts:selected-signature:v2");
    hash_invocation_field(
        &mut hasher,
        match kind {
            crate::CallKind::Call => "call",
            crate::CallKind::Construct => "construct",
            crate::CallKind::Unknown => "unknown",
        },
    );
    hash_invocation_field(&mut hasher, &signature.declaration.symbol);
    hash_invocation_field(&mut hasher, &signature.declaration.location.path);
    hash_invocation_field(
        &mut hasher,
        &signature.declaration.location.start_byte.to_string(),
    );
    hash_invocation_field(
        &mut hasher,
        &signature.declaration.location.end_byte.to_string(),
    );
    hash_invocation_field(&mut hasher, &signature.overload_ordinal.to_string());
    hash_invocation_field(&mut hasher, &signature.overload_count.to_string());
    hash_invocation_field(&mut hasher, &signature.minimum_argument_count.to_string());
    hash_invocation_field(
        &mut hasher,
        if signature.has_rest { "true" } else { "false" },
    );
    for parameter in &signature.parameters {
        hash_invocation_field(
            &mut hasher,
            parameter
                .value
                .type_descriptor
                .as_ref()
                .map_or("", |descriptor| &descriptor.text),
        );
    }
    hash_invocation_field(
        &mut hasher,
        signature
            .result
            .type_descriptor
            .as_ref()
            .map_or("", |descriptor| &descriptor.text),
    );
    format!("sha256:{:x}", hasher.finalize())
}

fn hash_invocation_field(hasher: &mut Sha256, value: &str) {
    hasher.update(value.len().to_string().as_bytes());
    hasher.update(b":");
    hasher.update(value.as_bytes());
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
        if candidate.schema() != v3::TYPE_FACTS_TABLE_SCHEMA_V17
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
        || transition.table_schema != v3::TYPE_FACTS_TABLE_SCHEMA_V17
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
mod tests {
    use super::*;
    use crate::{EntityFact, Location, SourceDigest, SourceHash, SymbolFact, v3::SymbolOp};
    use serde::Deserialize;

    #[test]
    fn project_identity_matches_the_producers_absolute_clean_path() {
        let current = std::env::current_dir().unwrap();
        let dirty = current
            .join("nested")
            .join("..")
            .join("project")
            .join(".")
            .join("tsconfig.json");
        let normalized = normalize_project_id(dirty.to_string_lossy().into_owned()).unwrap();
        assert_eq!(
            normalized,
            current.join("project/tsconfig.json").to_string_lossy()
        );

        let relative = normalize_project_id("project/../tsconfig.json".into()).unwrap();
        assert_eq!(relative, current.join("tsconfig.json").to_string_lossy());
        assert!(normalize_project_id("  ".into()).is_err());
    }

    fn location(path: &str, start: u64) -> Location {
        Location {
            path: path.into(),
            start_byte: start,
            end_byte: start + 1,
        }
    }

    #[test]
    fn invocation_completeness_and_generation_identity_fail_closed_locally() {
        let transcript = crate::InvocationTranscript {
            location: location("/p/source.ts", 0),
            validity: crate::ResolvedCallValidity::Unresolved,
            kind: crate::CallKind::Unknown,
            target: "".into(),
            targets: None,
            selected_signature: None,
            bindings: Vec::new(),
            omitted_parameters: Vec::new(),
            parameter_uses: Vec::new(),
            control_flow: None,
            completeness: crate::InvocationCompleteness(vec![
                crate::InvocationDomain::Bindings,
                crate::InvocationDomain::Bindings,
            ]),
            open_reasons: Vec::new(),
        };
        assert!(matches!(
            validate_invocation_transcript(&transcript),
            Err(SessionError::InvalidResponse(_))
        ));

        let transcript = crate::InvocationTranscript {
            completeness: crate::InvocationCompleteness(vec![crate::InvocationDomain::Signature]),
            ..transcript
        };
        assert!(matches!(
            validate_invocation_transcript(&transcript),
            Err(SessionError::InvalidResponse(_))
        ));

        let demands = Vec::<crate::InvocationDemand>::new();
        let envelope = crate::InvocationEnvelope {
            project_id: "/p/tsconfig.json".into(),
            generation: 7,
            demand_sha256: invocation_demand_digest(&demands).into(),
            module_graph_sha256: crate::SourceHash::of("module graph").as_str().into(),
            schema_sha256: v3::TYPE_FACTS_SCHEMA_SHA256.into(),
            producer_build: v3::TYPE_FACTS_BUILD_ID.into(),
            sources: Vec::new(),
            open_reasons: Vec::new(),
        };
        assert!(validate_invocation_envelope(&envelope, "/p/tsconfig.json", 7, &demands,).is_ok());
        assert!(matches!(
            validate_invocation_envelope(&envelope, "/p/tsconfig.json", 8, &demands),
            Err(SessionError::InvalidResponse(_))
        ));

        let mut stale_producer = envelope;
        stale_producer.producer_build = "stale-build".into();
        assert!(matches!(
            validate_invocation_envelope(&stale_producer, "/p/tsconfig.json", 7, &demands,),
            Err(SessionError::InvalidResponse(_))
        ));
    }

    #[test]
    fn invocation_demand_digest_matches_the_go_producer_fixture() {
        let demands = [
            crate::InvocationDemand {
                location: Location {
                    path: "/p/a.ts".into(),
                    start_byte: 4,
                    end_byte: 12,
                },
                callable_depth: 3,
                census: true,
            },
            crate::InvocationDemand {
                location: Location {
                    path: "/p/b.ts".into(),
                    start_byte: 0,
                    end_byte: 7,
                },
                callable_depth: 0,
                census: false,
            },
        ];
        assert_eq!(
            invocation_demand_digest(&demands),
            "sha256:68a28ae5071bb694387c8d372c3a3febd48636f783d4a5881ece0a15656fb88c"
        );
        let reversed = [demands[1].clone(), demands[0].clone()];
        assert_ne!(
            invocation_demand_digest(&reversed),
            invocation_demand_digest(&demands)
        );
    }

    #[test]
    fn shared_demand_groups_retain_the_callers_allocation() {
        let run: Arc<[EntityDemand]> = vec![EntityDemand {
            location: location("/p/a.ts", 1),
            symbol: true,
            ..EntityDemand::default()
        }]
        .into();
        let shared = DemandGroup::shared(&run).expect("non-empty shared run");
        let retained = shared.retained();

        assert!(Arc::ptr_eq(&run, &retained));
        assert_eq!(retained.as_ref(), shared.demands());
    }

    fn table_with_symbol(symbol: SymbolFact) -> FactTable {
        fact_table(Vec::new(), Vec::new(), vec![symbol], Vec::new())
    }

    fn fact_table(
        sources: Vec<SourceDigest>,
        entities: Vec<EntityFact>,
        symbols: Vec<SymbolFact>,
        files: Vec<crate::FileFact>,
    ) -> FactTable {
        FactTable::from_parts(
            v3::TYPE_FACTS_TABLE_SCHEMA_V17,
            1,
            "/p/tsconfig.json",
            sources,
            entities,
            symbols,
            files,
        )
    }

    fn delta_transition(
        base_generation: u64,
        target_generation: u64,
        paths: Vec<v3::PathOp>,
        symbols: Vec<SymbolOp>,
    ) -> WireTableTransition {
        WireTableTransition {
            mode: TransitionMode::Delta,
            table_schema: v3::TYPE_FACTS_TABLE_SCHEMA_V17,
            base_generation,
            target_generation,
            project_id: "/p/tsconfig.json".into(),
            base_state_token: format!("base-{base_generation}").into(),
            paths,
            symbols,
        }
    }

    fn reference_transition(
        id: &str,
        path: &str,
        references: Vec<Location>,
    ) -> WireTableTransition {
        delta_transition(
            1,
            2,
            Vec::new(),
            vec![SymbolOp::ReplaceReferencePath {
                id: id.into(),
                path: path.into(),
                references,
            }],
        )
    }

    fn response(generation: u64, state_token: &str, table_transition: Vec<u8>) -> Response {
        Response {
            schema: v3::TYPE_FACTS_SCHEMA_V1,
            request_id: 1,
            project_id: "/p/tsconfig.json".into(),
            generation,
            ok: true,
            table_transition,
            symbol_evidence: Vec::new(),
            reference_evidence: Vec::new(),
            changed_reference_symbols: Vec::new(),
            reference_changes_exact: false,
            state_token: state_token.into(),
            affected: Vec::new(),
            sources: Vec::new(),
            source_arena: String::new(),
            source_lengths: Vec::new(),
            timings: None,
            error: None,
            modules: Vec::new(),
            module_imports: Vec::new(),
            unknown_import_paths: Vec::new(),
            invocation_transcripts: Vec::new(),
            invocation_envelope: None,
            client_decode_ns: 0,
            client_response_bytes: 0,
            client_request_send_ns: 0,
            client_request_bytes: 0,
            client_roundtrip_ns: 0,
        }
    }

    fn materialize_full(frame: &[u8], label: &str) -> FactTable {
        let transition = v3::decode_table_transition(frame)
            .unwrap_or_else(|error| panic!("{label}: decode full transition: {error}"));
        assert_eq!(
            transition.mode,
            TransitionMode::Full,
            "{label}: expected full transition"
        );
        materialize_full_transition(transition).0
    }

    #[test]
    fn repeated_edits_to_one_path_keep_only_the_newest_replay_overlay() {
        // A long editing session sends the same handful of files over and over.
        // Retaining every version would grow the session by the full source text
        // of each edit; only the newest overlay per path changes what a replayed
        // generation produces.
        let mut session = Session {
            producer: Producer::at("/nonexistent"),
            project_id: "/p/tsconfig.json".into(),
            generation: 1,
            session_id: crate::SourceHash::of("test-session"),
            restart_epoch: 0,
            connection: None,
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
        };
        for version in 1..=32 {
            let changes = vec![FileChange {
                path: "/p/a.ts".into(),
                version,
                source: "x".repeat(1024).into_bytes(),
                deleted: false,
            }];
            session.supersede_replayed_overlays(&changes);
            session.replay_batches.push(changes);
        }

        // One batch per accepted update, because the producer advances a
        // generation per update and replay must land on the same generation.
        assert_eq!(session.replay_batches.len(), 32);
        let retained: usize = session.replay_batches.iter().map(Vec::len).sum();
        assert_eq!(
            retained, 1,
            "only the newest overlay for a path may be retained, found {retained}"
        );
        let newest = session
            .replay_batches
            .last()
            .and_then(|batch| batch.first())
            .expect("the newest batch keeps its overlay");
        assert_eq!(newest.version, 32);
    }

    #[test]
    fn certification_pin_is_lost_when_the_launch_recipe_changes() {
        let executable = std::env::current_exe().unwrap();
        let executable_sha256 = sha256_file(&executable).unwrap();
        let source_manifest = crate::SourceHash::of("source-manifest");
        let pinned = Producer::pinned_for_certification(
            &executable,
            executable_sha256.to_string(),
            source_manifest.to_string(),
        )
        .unwrap();
        assert!(pinned.certification_pin.is_some());
        assert!(
            pinned
                .clone()
                .with_arg("--shim")
                .certification_pin
                .is_none()
        );
        assert!(
            pinned
                .without_shared_transition_arena()
                .certification_pin
                .is_none()
        );
    }

    /// A reference replacement touches only its own path's run and leaves the
    /// surrounding path-sorted order intact.
    #[test]
    fn reference_transition_replaces_only_the_named_paths_run() {
        let mut table = table_with_symbol(SymbolFact {
            id: "shared".into(),
            alias_target: "".into(),
            declarations: Vec::new().into(),
            references: vec![
                location("a.ts", 1),
                location("b.ts", 1),
                location("c.ts", 1),
            ]
            .into(),
        });
        let transition = reference_transition(
            "shared",
            "b.ts",
            vec![location("b.ts", 4), location("b.ts", 9)],
        );
        let changes =
            apply_table_transition(&mut table, transition).expect("apply the reference transition");

        assert_eq!(table.generation(), 2);
        assert_eq!(
            table
                .symbol("shared")
                .expect("shared symbol")
                .references()
                .map(|reference| (reference.path.as_ref(), reference.start_byte))
                .collect::<Vec<_>>(),
            vec![("a.ts", 1), ("b.ts", 4), ("b.ts", 9), ("c.ts", 1)],
        );
        assert_eq!(
            changes,
            TableChanges {
                unchanged: false,
                entity_paths: Vec::new(),
                symbol_ids: vec!["shared".into()],
                file_paths: Vec::new(),
            }
        );
    }

    /// Rows keyed by path or id are replaced, removed, and re-sorted.
    #[test]
    fn keyed_rows_are_replaced_removed_and_reordered() {
        let mut table = fact_table(
            vec![
                SourceDigest {
                    path: "a.ts".into(),
                    sha256: SourceHash::of("a"),
                },
                SourceDigest {
                    path: "b.ts".into(),
                    sha256: SourceHash::of("b"),
                },
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let transition = delta_transition(
            1,
            2,
            vec![
                v3::PathOp {
                    path: "a.ts".into(),
                    source: SlotOp::Replace(SourceHash::of("a2")),
                    entities: SlotOp::Unchanged,
                    file: SlotOp::Unchanged,
                },
                v3::PathOp {
                    path: "b.ts".into(),
                    source: SlotOp::Remove,
                    entities: SlotOp::Unchanged,
                    file: SlotOp::Unchanged,
                },
            ],
            Vec::new(),
        );
        apply_table_transition(&mut table, transition).expect("apply the keyed transition");

        assert_eq!(table.source_count(), 1);
        let source = table.source("a.ts").expect("retained a.ts source");
        assert_eq!(source.path.as_ref(), "a.ts");
        assert_eq!(source.sha256, SourceHash::of("a2"));
    }

    #[test]
    fn invalid_candidate_never_mutates_the_retained_table() {
        let retained = fact_table(
            vec![SourceDigest {
                path: "a.ts".into(),
                sha256: SourceHash::of("old"),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let transition = delta_transition(
            1,
            2,
            vec![v3::PathOp {
                path: "a.ts".into(),
                source: SlotOp::Replace(SourceHash::of("new")),
                entities: SlotOp::Unchanged,
                file: SlotOp::Unchanged,
            }],
            vec![SymbolOp::ReplaceReferencePath {
                id: "missing".into(),
                path: "a.ts".into(),
                references: vec![location("a.ts", 1)],
            }],
        );
        let mut candidate = retained.clone();
        let candidate_root = candidate.path_root_identity();
        assert!(matches!(
            apply_table_transition(&mut candidate, transition),
            Err(SessionError::InvalidResponse(_))
        ));
        assert_eq!(
            candidate.path_root_identity(),
            candidate_root,
            "a rejected transition copied storage before validation finished"
        );
        assert_eq!(candidate, retained);
        assert_eq!(
            retained.source("a.ts").expect("a.ts").sha256,
            SourceHash::of("old")
        );
        assert_eq!(retained.generation(), 1);
    }

    #[test]
    fn sparse_transition_copies_only_the_touched_leaf() {
        let table = || {
            fact_table(
                (0..128)
                    .map(|index| SourceDigest {
                        path: format!("file-{index:03}.ts").into(),
                        sha256: SourceHash::of("old"),
                    })
                    .collect(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        };
        let transition = delta_transition(
            1,
            2,
            vec![v3::PathOp {
                path: "file-000.ts".into(),
                source: SlotOp::Replace(SourceHash::of("new")),
                entities: SlotOp::Unchanged,
                file: SlotOp::Unchanged,
            }],
            Vec::new(),
        );

        let mut shared = table();
        let held = shared.clone();
        let touched_leaf = shared.path_leaf_identity("file-000.ts");
        let untouched_leaf = shared.path_leaf_identity("file-127.ts");
        apply_table_transition(&mut shared, transition).unwrap();
        assert_ne!(shared.path_leaf_identity("file-000.ts"), touched_leaf);
        assert_eq!(shared.path_leaf_identity("file-127.ts"), untouched_leaf);
        assert_eq!(
            shared.source("file-000.ts").expect("new source").sha256,
            SourceHash::of("new")
        );
        assert_eq!(
            held.source("file-000.ts").expect("held source").sha256,
            SourceHash::of("old")
        );
    }

    #[test]
    fn broad_transition_uses_one_canonical_merge() {
        const BROAD_OPERATION_COUNT: usize = 32;
        let sources = (0..64)
            .map(|index| {
                let path: Arc<str> = format!("file-{index:03}.ts").into();
                SourceDigest {
                    path,
                    sha256: SourceHash::of("old"),
                }
            })
            .collect::<Vec<_>>();
        let paths = (0..BROAD_OPERATION_COUNT)
            .map(|index| v3::PathOp {
                path: format!("file-{index:03}.ts").into(),
                source: SlotOp::Replace(SourceHash::of("new")),
                entities: SlotOp::Unchanged,
                file: SlotOp::Unchanged,
            })
            .collect();
        let mut table = fact_table(sources, Vec::new(), Vec::new(), Vec::new());
        let original_root = table.path_root_identity();

        apply_table_transition(&mut table, delta_transition(1, 2, paths, Vec::new()))
            .expect("apply the broad transition");

        assert_ne!(
            table.path_root_identity(),
            original_root,
            "the broad branch should build one merged index"
        );
        assert_eq!(table.source_count(), 64);
        let sources = table.sources().collect::<Vec<_>>();
        assert!(sources.windows(2).all(|pair| pair[0].path < pair[1].path));
        assert!(
            sources[..BROAD_OPERATION_COUNT]
                .iter()
                .all(|source| source.sha256 == SourceHash::of("new"))
        );
        assert!(
            sources[BROAD_OPERATION_COUNT..]
                .iter()
                .all(|source| source.sha256 == SourceHash::of("old"))
        );
    }

    #[test]
    fn reuse_requires_the_retained_table_generation() {
        let retained = fact_table(Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let valid = response(1, "successor", Vec::new());
        let mut valid_retained = Some(retained.clone());
        let (reused, changes) =
            prepare_analyze_response(&valid, "/p/tsconfig.json", 1, &mut valid_retained, "base")
                .expect("reuse the retained table");
        assert_eq!(reused, retained);
        assert!(changes.unchanged);

        let invalid = response(2, "successor", Vec::new());
        let mut invalid_retained = Some(retained.clone());
        assert!(matches!(
            prepare_analyze_response(
                &invalid,
                "/p/tsconfig.json",
                2,
                &mut invalid_retained,
                "base"
            ),
            Err(SessionError::InvalidResponse(_))
        ));
        assert_eq!(invalid_retained.as_ref(), Some(&retained));
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct TransitionGoldenStep {
        label: String,
        base_token: String,
        #[serde(with = "serde_bytes")]
        base_transition: Vec<u8>,
        #[serde(with = "serde_bytes")]
        transition: Vec<u8>,
        #[serde(with = "serde_bytes")]
        expected_transition: Vec<u8>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct TransitionGolden {
        steps: Vec<TransitionGoldenStep>,
    }

    /// The production Go encoder and this decoder/applier must assign exactly
    /// the same meaning to full and delta transitions.
    #[test]
    fn applies_the_producers_transitions_exactly() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../benchmarks/typefacts/phase1/typefacts-v5-transition-golden.cbor");
        let bytes =
            std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let golden: TransitionGolden = crate::decode(&bytes).expect("decode the transition golden");
        assert_eq!(golden.steps.len(), 4, "fixture lost a transition");

        for step in &golden.steps {
            let mut actual = materialize_full(&step.base_transition, &step.label);
            let expected = materialize_full(&step.expected_transition, &step.label);
            let transition = v3::decode_table_transition(&step.transition)
                .unwrap_or_else(|error| panic!("{}: decode delta: {error}", step.label));
            assert_eq!(transition.mode, TransitionMode::Delta, "{}", step.label);
            assert_eq!(
                transition.base_state_token.as_ref(),
                step.base_token,
                "{}",
                step.label
            );
            assert_eq!(
                actual.generation(),
                transition.base_generation,
                "{}",
                step.label
            );
            apply_table_transition(&mut actual, transition)
                .unwrap_or_else(|error| panic!("{}: apply delta: {error}", step.label));
            assert_eq!(actual, expected, "{} produced the wrong table", step.label);
        }
    }
}
