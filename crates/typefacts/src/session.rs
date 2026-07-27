use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    io::BufWriter,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use thiserror::Error;

use crate::{
    EntityFact, FactTable, TypeFactsError, decode, decode_trusted, encode_sidecar_request,
    read_frame,
    v3::{
        self, EntityDemand, FactTableDelta, FileChange, Handshake, Operation, Request, Response,
        SourceFile,
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
}

impl Producer {
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            args: Vec::new(),
        }
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
}

impl<'a> DemandGroup<'a> {
    /// Borrows one file's demand run. Returns `None` for an empty run, which has
    /// no path and therefore no group.
    #[must_use]
    pub fn new(demands: &'a [EntityDemand]) -> Option<Self> {
        if demands.is_empty() {
            return None;
        }
        Some(Self { demands })
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
    writer: Arc<Mutex<BufWriter<ChildStdin>>>,
    pending: PendingResponses,
    next_request_id: Arc<AtomicU64>,
    active_request_id: Arc<AtomicU64>,
    reader: Option<JoinHandle<()>>,
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
        let mut child = Command::new(&producer.path)
            .args(&producer.args)
            .args(["-project", project_id])
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
                        response.client_decode_ns =
                            u64::try_from(decode_started.elapsed().as_nanos()).unwrap_or(u64::MAX);
                        response.client_response_bytes =
                            u64::try_from(payload.len()).unwrap_or(u64::MAX);
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
            writer,
            pending,
            next_request_id: Arc::new(AtomicU64::new(1)),
            active_request_id: Arc::new(AtomicU64::new(0)),
            reader: Some(reader),
        })
    }

    /// A request written to the producer whose response has not been collected.
    ///
    /// Holding one of these is what lets a caller overlap its own work with an
    /// acknowledgement. It is private: the session decides when a response is
    /// collected, so no consumer can leave one outstanding.
    fn send(&self, mut request: Request) -> Result<SentRequest, SessionError> {
        request.request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let request_id = request.request_id;
        let cancellable = request.operation == Operation::Analyze;
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
            let payload = encode_sidecar_request(&request)?;
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

    fn exchange(&self, request: Request) -> Result<Response, SessionError> {
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
/// Framing, request identities, handshake validation, retained table deltas,
/// and subprocess recovery are private implementation details.
pub struct Session {
    producer: Producer,
    project_id: String,
    generation: u64,
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
    retained_demands: HashMap<String, Vec<EntityDemand>>,
    retained_table: Option<FactTable>,
    last_exchange_timings: Option<ExchangeTimings>,
    last_update_timings: Option<UpdateTimings>,
    last_table_changes: Option<TableChanges>,
    closed: bool,
}

impl Session {
    pub fn open<I>(
        producer: Producer,
        project_id: impl Into<String>,
        sources: I,
    ) -> Result<Self, SessionError>
    where
        I: IntoIterator<Item = FileChange>,
    {
        let project_id = project_id.into();
        if project_id.trim().is_empty() {
            return Err(SessionError::InvalidResponse(
                "project identity is empty".into(),
            ));
        }
        let connection = Connection::spawn(&producer, &project_id)?;
        let mut session = Self {
            producer,
            project_id,
            generation: 1,
            connection: Some(connection),
            replay_batches: Vec::new(),
            replay_index: HashMap::new(),
            state_token: String::new(),
            retained_demands: HashMap::new(),
            retained_table: None,
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
                    .is_some_and(|retained| retained.as_slice() == group.demands());
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

        match self.analyze_exchange(wire_demands, removed.clone(), reset_state) {
            Err(SessionError::Service { code, .. }) if code == "state-mismatch" => {
                // The producer lost the state this delta was relative to, so the
                // next request must carry the complete demand set.
                self.clear_retained_state();
                Self::reject_foreign_locations(groups)?;
                let complete = groups
                    .iter()
                    .flat_map(|group| group.demands().iter().cloned())
                    .collect::<Vec<_>>();
                let table = self.analyze_exchange(complete, Vec::new(), true)?;
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
            match self.retained_demands.get_mut(group.path()) {
                // Reuse the existing allocation rather than freeing it and taking
                // a new one for a run that is usually the same length.
                Some(retained) => {
                    retained.clear();
                    retained.extend_from_slice(group.demands());
                }
                None => {
                    self.retained_demands
                        .insert(group.path().to_owned(), group.demands().to_vec());
                }
            }
        }
    }

    fn retain_all_groups(&mut self, groups: &[DemandGroup<'_>]) {
        self.retained_demands.clear();
        for group in groups {
            self.retained_demands
                .insert(group.path().to_owned(), group.demands().to_vec());
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
            Ok(_) => {
                self.commit_update(changes);
                Ok(())
            }
            Err(error) if error.is_transport_failure() => {
                // This update is not in the replay state yet, so the replay
                // restores everything before it and it is re-sent exactly once.
                self.restart_and_replay()?;
                let mut retry = request(Operation::Update, &self.project_id, self.generation + 1);
                retry.changes.clone_from(&changes);
                self.exchange_once(retry)?;
                self.commit_update(changes);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Records an acknowledged update: one generation, one replay batch.
    fn commit_update(&mut self, changes: Vec<FileChange>) {
        self.generation += 1;
        self.supersede_replayed_overlays(&changes);
        self.replay_batches.push(changes);
    }

    fn send_once(&self, sending: Request) -> Result<SentRequest, SessionError> {
        self.connection
            .as_ref()
            .ok_or(SessionError::Closed)?
            .send(sending)
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

    pub fn close(&mut self) -> Result<(), SessionError> {
        if self.closed {
            return Ok(());
        }
        let result =
            self.exchange_once(request(Operation::Close, &self.project_id, self.generation));
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
        let mut response = self.exchange(analyze)?;
        // Deltas travel as the opaque packed frame; expand it once here so
        // everything downstream keeps working on the semantic delta.
        if !response.packed_delta.is_empty() {
            response.table_delta = Some(
                v3::decode_packed_fact_table_delta(&response.packed_delta)
                    .map_err(SessionError::InvalidResponse)?,
            );
        }
        self.last_exchange_timings = Some(exchange_timings(&response));
        self.last_table_changes = Some(table_changes(&response)?);
        let table = match response.table_mode.as_str() {
            "full" => {
                if response.packed_table.is_empty() {
                    return Err(SessionError::InvalidResponse(
                        "full response has no packed table".into(),
                    ));
                }
                v3::decode_packed_fact_table(&response.packed_table, response.project_id.clone())
                    .map_err(|error| SessionError::InvalidResponse(error.to_string()))?
            }
            // Both retained modes take the table rather than cloning it: the
            // retained copy is replaced below anyway, so cloning here would deep
            // copy every entity and symbol in the project twice per analysis.
            "reuse" => {
                let mut table = self.retained_table.take().ok_or_else(|| {
                    SessionError::InvalidResponse("reuse response has no retained table".into())
                })?;
                table.generation = response.generation;
                table.project_id.clone_from(&response.project_id);
                table
            }
            "delta" => {
                let mut table = self.retained_table.take().ok_or_else(|| {
                    SessionError::InvalidResponse("delta response has no retained table".into())
                })?;
                let delta = match response.table_delta.as_ref() {
                    Some(delta) => delta,
                    None => {
                        self.clear_retained_state();
                        return Err(SessionError::InvalidResponse(
                            "delta response has no delta".into(),
                        ));
                    }
                };
                if let Err(error) = apply_table_delta(&mut table, delta) {
                    // The table was taken and is now of unknown shape, so the
                    // retained state it belonged to is no longer trustworthy.
                    // Failing closed here makes the next analysis a full reset
                    // instead of a delta against something half-applied.
                    self.clear_retained_state();
                    return Err(error);
                }
                table
            }
            other => {
                return Err(SessionError::InvalidResponse(format!(
                    "unsupported table mode {other:?}"
                )));
            }
        };
        if table.project_id != response.project_id || table.generation != response.generation {
            return Err(SessionError::InvalidResponse(
                "table identity does not match response".into(),
            ));
        }
        if response.state_token.is_empty() {
            return Err(SessionError::InvalidResponse(
                "retained response has no state token".into(),
            ));
        }
        self.state_token = response.state_token;
        self.retained_table = Some(table.clone());
        Ok(table)
    }

    fn exchange(&mut self, request: Request) -> Result<Response, SessionError> {
        match self.exchange_once(request.clone()) {
            Err(error) if error.is_transport_failure() => {
                self.restart_and_replay()?;
                self.exchange_once(request)
            }
            result => result,
        }
    }

    fn exchange_once(&self, request: Request) -> Result<Response, SessionError> {
        self.connection
            .as_ref()
            .ok_or(SessionError::Closed)?
            .exchange(request)
    }

    fn restart_and_replay(&mut self) -> Result<(), SessionError> {
        if let Some(mut connection) = self.connection.take() {
            connection.terminate();
        }
        self.connection = Some(Connection::spawn(&self.producer, &self.project_id)?);
        self.generation = 1;
        self.clear_retained_state();
        self.exchange_once(request(Operation::Open, &self.project_id, 1))?;
        for changes in self.replay_batches.clone() {
            let mut update = request(Operation::Update, &self.project_id, self.generation + 1);
            update.changes = changes;
            self.exchange_once(update)?;
            self.generation += 1;
        }
        Ok(())
    }

    fn clear_retained_state(&mut self) {
        self.state_token.clear();
        self.retained_demands.clear();
        self.retained_table = None;
    }

    fn ensure_open(&self) -> Result<(), SessionError> {
        if self.closed {
            Err(SessionError::Closed)
        } else {
            Ok(())
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn request(operation: Operation, project_id: &str, generation: u64) -> Request {
    Request {
        schema: v3::TYPE_FACTS_SCHEMA_V3,
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
        cancel_request_id: 0,
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

fn table_changes(response: &Response) -> Result<TableChanges, SessionError> {
    match response.table_mode.as_str() {
        "reuse" => Ok(TableChanges {
            unchanged: true,
            ..TableChanges::default()
        }),
        "delta" => {
            let delta = response.table_delta.as_ref().ok_or_else(|| {
                SessionError::InvalidResponse("delta response has no delta".into())
            })?;
            let mut entity_paths = delta
                .entity_files
                .iter()
                .map(|file| file.path.clone())
                .chain(delta.removed_entity_paths.iter().cloned())
                .collect::<Vec<_>>();
            let mut symbol_ids = delta
                .symbols
                .iter()
                .map(|symbol| symbol.id.to_string())
                .chain(
                    delta
                        .symbol_reference_files
                        .iter()
                        .map(|references| references.id.clone()),
                )
                .chain(delta.removed_symbol_ids.iter().cloned())
                .collect::<Vec<_>>();
            let mut file_paths = delta
                .files
                .iter()
                .map(|file| file.path.to_string())
                .chain(delta.removed_file_paths.iter().cloned())
                .collect::<Vec<_>>();
            entity_paths.sort();
            entity_paths.dedup();
            symbol_ids.sort();
            symbol_ids.dedup();
            file_paths.sort();
            file_paths.dedup();
            let unchanged =
                entity_paths.is_empty() && symbol_ids.is_empty() && file_paths.is_empty();
            Ok(TableChanges {
                unchanged,
                entity_paths,
                symbol_ids,
                file_paths,
            })
        }
        _ => Ok(TableChanges::default()),
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

/// Replaces or inserts one row of a vector kept sorted and unique by `key`.
fn upsert_sorted_row<T: Clone>(rows: &mut Vec<T>, row: &T, key: impl Fn(&T) -> &str) {
    match rows.binary_search_by(|value| key(value).cmp(key(row))) {
        Ok(index) => rows[index] = row.clone(),
        Err(index) => rows.insert(index, row.clone()),
    }
}

/// Removes the row carrying `removed` from a vector kept sorted and unique by
/// `key`. A miss is fine: removal of a row the client never demanded.
fn remove_sorted_row<T>(rows: &mut Vec<T>, removed: &str, key: impl Fn(&T) -> &str) {
    if let Ok(index) = rows.binary_search_by(|value| key(value).cmp(removed)) {
        rows.remove(index);
    }
}

/// The bounds of one path's contiguous run in the path-major entity order.
fn entity_run(entities: &[EntityFact], path: &str) -> std::ops::Range<usize> {
    let start = entities.partition_point(|entity| entity.location.path.as_ref() < path);
    let end = entities.partition_point(|entity| entity.location.path.as_ref() <= path);
    start..end
}

/// Applies a delta by splicing each changed row or run into its place in the
/// retained table's canonical order. Every section is ordered and the delta
/// names only the rows that may differ, so nothing here scans, hashes, or
/// re-sorts the unchanged remainder — the previous retain/extend/sort walked
/// the entire table per generation.
fn apply_table_delta(table: &mut FactTable, delta: &FactTableDelta) -> Result<(), SessionError> {
    let sources = Arc::make_mut(&mut table.sources);
    for removed in &delta.removed_source_paths {
        remove_sorted_row(sources, removed, |value| value.path.as_ref());
    }
    for source in &delta.sources {
        upsert_sorted_row(sources, source, |value| value.path.as_ref());
    }

    let entities = Arc::make_mut(&mut table.entities);
    for removed in &delta.removed_entity_paths {
        let run = entity_run(entities, removed);
        entities.drain(run);
    }
    for file in &delta.entity_files {
        // The splice below replaces the path's whole run in place, so the
        // table only stays canonically ordered if the replacement is a valid
        // run itself: every entity on the named path, in span order. The
        // producer emits it that way; check rather than assume, because an
        // unordered splice would corrupt every later delta.
        if file
            .entities
            .iter()
            .any(|entity| entity.location.path.as_ref() != file.path.as_str())
        {
            return Err(SessionError::InvalidResponse(format!(
                "entity delta for {:?} contains another path",
                file.path
            )));
        }
        if !file.entities.windows(2).all(|pair| {
            (pair[0].location.start_byte, pair[0].location.end_byte)
                <= (pair[1].location.start_byte, pair[1].location.end_byte)
        }) {
            return Err(SessionError::InvalidResponse(format!(
                "entity delta for {:?} is not in canonical order",
                file.path
            )));
        }
        let run = entity_run(entities, &file.path);
        entities.splice(run, file.entities.iter().cloned());
    }

    let symbols = Arc::make_mut(&mut table.symbols);
    for removed in &delta.removed_symbol_ids {
        remove_sorted_row(symbols, removed, |value| value.id.as_ref());
    }
    for symbol in &delta.symbols {
        upsert_sorted_row(symbols, symbol, |value| value.id.as_ref());
    }
    for replacement in &delta.symbol_reference_files {
        if replacement
            .references
            .iter()
            .any(|reference| reference.path.as_ref() != replacement.path.as_str())
        {
            return Err(SessionError::InvalidResponse(format!(
                "reference delta for {:?} contains another path",
                replacement.path
            )));
        }
        // The splice below locates the path's run with `partition_point`, so a
        // retained list only stays canonically ordered if the replacement is
        // ordered too. The producer emits it that way; check rather than
        // assume, because an unsorted splice would corrupt every later delta.
        if !replacement.references.windows(2).all(|pair| {
            (pair[0].start_byte, pair[0].end_byte) <= (pair[1].start_byte, pair[1].end_byte)
        }) {
            return Err(SessionError::InvalidResponse(format!(
                "reference delta for {:?} is not in canonical order",
                replacement.path
            )));
        }
        let symbol_index = symbols
            .binary_search_by(|symbol| symbol.id.as_ref().cmp(replacement.id.as_str()))
            .map_err(|_| {
                SessionError::InvalidResponse(format!(
                    "reference delta names missing symbol {:?}",
                    replacement.id
                ))
            })?;
        let symbol = &mut symbols[symbol_index];
        // The reference list is shared with older generations, so replace the
        // path's run by rebuilding the list — sized once, no splice-shifting.
        let references = &symbol.references;
        let start = references
            .partition_point(|reference| reference.path.as_ref() < replacement.path.as_str());
        let end = references
            .partition_point(|reference| reference.path.as_ref() <= replacement.path.as_str());
        let mut next =
            Vec::with_capacity(references.len() - (end - start) + replacement.references.len());
        next.extend_from_slice(&references[..start]);
        next.extend(replacement.references.iter().cloned());
        next.extend_from_slice(&references[end..]);
        symbol.references = next.into();
    }

    let files = Arc::make_mut(&mut table.files);
    for removed in &delta.removed_file_paths {
        remove_sorted_row(files, removed, |value| value.path.as_ref());
    }
    for file in &delta.files {
        upsert_sorted_row(files, file, |value| value.path.as_ref());
    }
    table.generation = delta.generation;
    Ok(())
}

fn decode_sources(response: Response) -> Result<Vec<SourceFile>, SessionError> {
    if response.source_arena.is_empty() {
        return Ok(response.sources);
    }
    let bytes = std::fs::read(&response.source_arena)
        .map_err(|error| SessionError::Process(format!("read source arena: {error}")))?;
    let _ = std::fs::remove_file(&response.source_arena);
    if response.source_lengths.len() != response.sources.len() {
        return Err(SessionError::InvalidResponse(
            "source arena descriptor count mismatch".into(),
        ));
    }
    let mut offset = 0usize;
    let mut sources = Vec::with_capacity(response.sources.len());
    for (mut source, length) in response.sources.into_iter().zip(response.source_lengths) {
        let length = usize::try_from(length)
            .map_err(|_| SessionError::InvalidResponse("source arena length overflow".into()))?;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| SessionError::InvalidResponse("source arena range overflow".into()))?;
        source.source = bytes
            .get(offset..end)
            .ok_or_else(|| {
                SessionError::InvalidResponse("source arena range is out of bounds".into())
            })?
            .to_vec();
        source.local = false;
        sources.push(source);
        offset = end;
    }
    if offset != bytes.len() {
        return Err(SessionError::InvalidResponse(
            "source arena has trailing bytes".into(),
        ));
    }
    Ok(sources)
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
    use crate::{Location, SourceDigest, SourceHash, SymbolFact};
    use serde::Deserialize;

    fn location(path: &str, start: u64) -> Location {
        Location {
            path: path.into(),
            start_byte: start,
            end_byte: start + 1,
        }
    }

    fn table_with_symbol(symbol: SymbolFact) -> FactTable {
        FactTable {
            schema: 2,
            generation: 1,
            project_id: "/p/tsconfig.json".into(),
            sources: Vec::new().into(),
            entities: Vec::new().into(),
            symbols: vec![symbol].into(),
            files: Vec::new().into(),
        }
    }

    fn reference_delta(id: &str, path: &str, references: Vec<Location>) -> FactTableDelta {
        FactTableDelta {
            generation: 2,
            symbol_reference_files: vec![v3::SymbolReferenceFileDelta {
                id: id.into(),
                path: path.into(),
                references,
            }],
            sources: Vec::new(),
            removed_source_paths: Vec::new(),
            entity_files: Vec::new(),
            removed_entity_paths: Vec::new(),
            symbols: Vec::new(),
            removed_symbol_ids: Vec::new(),
            files: Vec::new(),
            removed_file_paths: Vec::new(),
        }
    }

    /// A reference replacement touches only its own path's run and leaves the
    /// surrounding path-sorted order intact.
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
            connection: None,
            replay_batches: Vec::new(),
            replay_index: HashMap::new(),
            state_token: String::new(),
            retained_demands: HashMap::new(),
            retained_table: None,
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
    fn reference_delta_replaces_only_the_named_paths_run() {
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
        apply_table_delta(
            &mut table,
            &reference_delta(
                "shared",
                "b.ts",
                vec![location("b.ts", 4), location("b.ts", 9)],
            ),
        )
        .expect("apply the reference delta");

        assert_eq!(table.generation, 2);
        assert_eq!(
            table.symbols[0]
                .references
                .iter()
                .map(|reference| (reference.path.as_ref(), reference.start_byte))
                .collect::<Vec<_>>(),
            vec![("a.ts", 1), ("b.ts", 4), ("b.ts", 9), ("c.ts", 1)],
        );
    }

    /// An empty replacement drops the path's references without disturbing
    /// the neighbours.
    #[test]
    fn empty_reference_delta_clears_only_that_path() {
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
        apply_table_delta(&mut table, &reference_delta("shared", "b.ts", Vec::new()))
            .expect("apply the empty reference delta");
        assert_eq!(
            table.symbols[0]
                .references
                .iter()
                .map(|reference| reference.path.as_ref())
                .collect::<Vec<_>>(),
            vec!["a.ts", "c.ts"],
        );
    }

    /// Both of these mean the client and producer disagree about the retained
    /// table, so the frame fails closed rather than corrupting it silently.
    #[test]
    fn reference_delta_fails_closed_on_desync() {
        let mut table = table_with_symbol(SymbolFact {
            id: "shared".into(),
            alias_target: "".into(),
            declarations: Vec::new().into(),
            references: vec![location("a.ts", 1)].into(),
        });
        assert!(matches!(
            apply_table_delta(
                &mut table.clone(),
                &reference_delta("missing", "a.ts", vec![location("a.ts", 2)]),
            ),
            Err(SessionError::InvalidResponse(_))
        ));
        assert!(matches!(
            apply_table_delta(
                &mut table.clone(),
                &reference_delta("shared", "a.ts", vec![location("elsewhere.ts", 2)]),
            ),
            Err(SessionError::InvalidResponse(_))
        ));
        assert!(matches!(
            apply_table_delta(
                &mut table,
                &reference_delta(
                    "shared",
                    "a.ts",
                    vec![location("a.ts", 9), location("a.ts", 2)],
                ),
            ),
            Err(SessionError::InvalidResponse(_))
        ));
    }

    /// Rows keyed by path or id are replaced, removed, and re-sorted.
    #[test]
    fn keyed_rows_are_replaced_removed_and_reordered() {
        let mut table = FactTable {
            schema: 2,
            generation: 1,
            project_id: "/p/tsconfig.json".into(),
            sources: vec![
                SourceDigest {
                    path: "a.ts".into(),
                    sha256: SourceHash::of("a"),
                },
                SourceDigest {
                    path: "b.ts".into(),
                    sha256: SourceHash::of("b"),
                },
            ]
            .into(),
            entities: Vec::new().into(),
            symbols: Vec::new().into(),
            files: Vec::new().into(),
        };
        let mut delta = reference_delta("unused", "a.ts", Vec::new());
        delta.symbol_reference_files.clear();
        delta.sources = vec![SourceDigest {
            path: "a.ts".into(),
            sha256: SourceHash::of("a2"),
        }];
        delta.removed_source_paths = vec!["b.ts".into()];
        apply_table_delta(&mut table, &delta).expect("apply the keyed delta");

        assert_eq!(table.sources.len(), 1);
        assert_eq!(table.sources[0].path.as_ref(), "a.ts");
        assert_eq!(table.sources[0].sha256, SourceHash::of("a2"));
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct DeltaGoldenStep {
        label: String,
        #[serde(with = "serde_bytes")]
        base: Vec<u8>,
        delta: FactTableDelta,
        #[serde(with = "serde_bytes")]
        packed: Vec<u8>,
        #[serde(with = "serde_bytes")]
        expected: Vec<u8>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct DeltaGolden {
        steps: Vec<DeltaGoldenStep>,
    }

    /// The authoritative check that this applier — the one the client actually
    /// runs — reproduces what the Go producer means by each delta. The fixture
    /// is emitted by the production differ in
    /// internal/typefacts/protocolv3_delta_golden_test.go.
    #[test]
    fn applies_the_producers_deltas_exactly() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmarks/phase1/typefacts-v3-delta-golden.cbor");
        let bytes =
            std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let golden: DeltaGolden = crate::decode(&bytes).expect("decode the delta golden");
        assert_eq!(golden.steps.len(), 4, "fixture lost a transition");

        for step in &golden.steps {
            let project = "/p/tsconfig.json".to_owned();
            let mut table = v3::decode_packed_fact_table(&step.base, project.clone())
                .unwrap_or_else(|error| panic!("{}: decode base: {error}", step.label));
            let expected = v3::decode_packed_fact_table(&step.expected, project)
                .unwrap_or_else(|error| panic!("{}: decode expected: {error}", step.label));

            // Responses carry the packed frame; it must expand to exactly the
            // semantic delta the fixture also pins in plain CBOR.
            let unpacked = v3::decode_packed_fact_table_delta(&step.packed)
                .unwrap_or_else(|error| panic!("{}: decode packed delta: {error}", step.label));
            assert_eq!(
                unpacked, step.delta,
                "{} packed frame expands to the wrong delta",
                step.label
            );

            apply_table_delta(&mut table, &unpacked)
                .unwrap_or_else(|error| panic!("{}: apply delta: {error}", step.label));

            assert_eq!(table, expected, "{} produced the wrong table", step.label);
        }
    }
}
