use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::Duration,
};

use thiserror::Error;

use crate::{
    FactTable, FramedTransport, Location, TypeFactsError, decode_trusted, encode_sidecar_request,
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
    pub structural_spans: Vec<Location>,
    pub compiler_spans: Vec<Location>,
    pub entities: Vec<EntityDemand>,
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

struct ProcessIo {
    input: ChildStdin,
    output: ChildStdout,
}

impl Read for ProcessIo {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.output.read(buffer)
    }
}

impl Write for ProcessIo {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.input.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.input.flush()
    }
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
        let transport = FramedTransport::new(ProcessIo { input, output });
        let (handshake_sender, handshake_receiver) = mpsc::sync_channel(1);
        let handshake_reader = std::thread::spawn(move || {
            let mut transport = transport;
            let handshake = transport.receive::<Handshake>();
            let _ = handshake_sender.send((handshake, transport));
        });
        let (handshake, transport) = match handshake_receiver.recv_timeout(Duration::from_secs(5)) {
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

        let ProcessIo { input, mut output } = transport.into_inner();
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
                let response = match decode_trusted::<Response>(&payload) {
                    Ok(response) => response,
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

    fn exchange(&self, mut request: Request) -> Result<Response, SessionError> {
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
        let result = (|| {
            let payload = encode_sidecar_request(&request)?;
            let mut writer = self
                .writer
                .lock()
                .map_err(|_| SessionError::Process("producer writer is poisoned".into()))?;
            write_frame(&mut *writer, &payload)?;
            Ok::<(), SessionError>(())
        })();
        if let Err(error) = result {
            if let Ok(mut pending) = self.pending.lock() {
                pending.remove(&request_id);
            }
            if cancellable {
                self.clear_active_request(request_id);
            }
            return Err(error);
        }
        let response = receiver
            .recv()
            .map_err(|_| SessionError::Process("producer response channel closed".into()))?
            .map_err(SessionError::Process);
        if cancellable {
            self.clear_active_request(request_id);
        }
        let response = response?;
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
    replay_batches: Vec<Vec<FileChange>>,
    state_token: String,
    retained_demands: HashMap<String, Vec<EntityDemand>>,
    retained_table: Option<FactTable>,
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
            state_token: String::new(),
            retained_demands: HashMap::new(),
            retained_table: None,
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

    pub fn analyze(&mut self, demand: &AnalysisDemand) -> Result<FactTable, SessionError> {
        self.ensure_open()?;
        let grouped = group_demands(&demand.entities);
        let reset_state = self.state_token.is_empty();
        let (wire_demands, removed_demand_paths) = if reset_state {
            (demand.entities.clone(), Vec::new())
        } else {
            demand_delta(&self.retained_demands, &grouped)
        };
        match self.analyze_exchange(demand, wire_demands, removed_demand_paths, reset_state) {
            Err(SessionError::Service { code, .. }) if code == "state-mismatch" => {
                self.clear_retained_state();
                let table =
                    self.analyze_exchange(demand, demand.entities.clone(), Vec::new(), true)?;
                self.retained_demands = grouped;
                Ok(table)
            }
            Ok(table) => {
                self.retained_demands = grouped;
                Ok(table)
            }
            Err(error) => Err(error),
        }
    }

    pub fn update<I>(&mut self, changes: I) -> Result<(), SessionError>
    where
        I: IntoIterator<Item = FileChange>,
    {
        self.ensure_open()?;
        let changes = changes.into_iter().collect::<Vec<_>>();
        if changes.is_empty() {
            return Ok(());
        }
        let mut update = request(Operation::Update, &self.project_id, self.generation + 1);
        update.changes.clone_from(&changes);
        self.exchange(update)?;
        self.generation += 1;
        self.replay_batches.push(changes);
        Ok(())
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
        demand: &AnalysisDemand,
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
        analyze
            .structural_spans
            .clone_from(&demand.structural_spans);
        analyze.compiler_spans.clone_from(&demand.compiler_spans);
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
        let table = match response.table_mode.as_str() {
            "full" => {
                if !response.packed_table.is_empty() {
                    v3::decode_packed_fact_table(
                        &response.packed_table,
                        response.project_id.clone(),
                    )
                    .map_err(|error| SessionError::InvalidResponse(error.to_string()))?
                } else {
                    match (response.table.take(), response.compact_table.take()) {
                        (Some(table), _) => table,
                        (None, Some(compact)) => compact
                            .expand()
                            .map_err(|error| SessionError::InvalidResponse(error.to_string()))?,
                        (None, None) => {
                            return Err(SessionError::InvalidResponse(
                                "full response has no table".into(),
                            ));
                        }
                    }
                }
            }
            "reuse" => {
                let mut table = self.retained_table.clone().ok_or_else(|| {
                    SessionError::InvalidResponse("reuse response has no retained table".into())
                })?;
                table.generation = response.generation;
                table.project_id.clone_from(&response.project_id);
                table
            }
            "delta" => {
                let mut table = self.retained_table.clone().ok_or_else(|| {
                    SessionError::InvalidResponse("delta response has no retained table".into())
                })?;
                apply_table_delta(
                    &mut table,
                    response.table_delta.as_ref().ok_or_else(|| {
                        SessionError::InvalidResponse("delta response has no delta".into())
                    })?,
                )?;
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
        structural_spans: Vec::new(),
        compiler_spans: Vec::new(),
        demands: Vec::new(),
        compact_demands: None,
        state_token: String::new(),
        reset_state: false,
        removed_demand_paths: Vec::new(),
        cancel_request_id: 0,
    }
}

fn group_demands(demands: &[EntityDemand]) -> HashMap<String, Vec<EntityDemand>> {
    let mut grouped: HashMap<String, Vec<_>> = HashMap::new();
    for demand in demands {
        grouped
            .entry(demand.location.path.clone())
            .or_default()
            .push(demand.clone());
    }
    grouped
}

fn demand_delta(
    previous: &HashMap<String, Vec<EntityDemand>>,
    next: &HashMap<String, Vec<EntityDemand>>,
) -> (Vec<EntityDemand>, Vec<String>) {
    let mut paths = next.keys().collect::<Vec<_>>();
    paths.sort();
    let mut changed = Vec::new();
    for path in paths {
        if previous.get(path) != next.get(path) {
            changed.extend(next[path].iter().cloned());
        }
    }
    let mut removed = previous
        .keys()
        .filter(|path| !next.contains_key(*path))
        .cloned()
        .collect::<Vec<_>>();
    removed.sort();
    (changed, removed)
}

fn apply_table_delta(table: &mut FactTable, delta: &FactTableDelta) -> Result<(), SessionError> {
    let source_paths = delta
        .sources
        .iter()
        .map(|value| value.path.as_str())
        .collect::<HashSet<_>>();
    let removed_source_paths = delta
        .removed_source_paths
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let sources = Arc::make_mut(&mut table.sources);
    sources.retain(|value| {
        !source_paths.contains(value.path.as_str())
            && !removed_source_paths.contains(value.path.as_str())
    });
    sources.extend(delta.sources.iter().cloned());
    sources.sort_by(|left, right| left.path.cmp(&right.path));

    let entity_paths = delta
        .entity_files
        .iter()
        .map(|value| value.path.as_str())
        .collect::<HashSet<_>>();
    let removed_entity_paths = delta
        .removed_entity_paths
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let entities = Arc::make_mut(&mut table.entities);
    entities.retain(|value| {
        !entity_paths.contains(value.location.path.as_str())
            && !removed_entity_paths.contains(value.location.path.as_str())
    });
    for file in &delta.entity_files {
        entities.extend(file.entities.iter().cloned());
    }
    entities.sort_by(|left, right| {
        (
            &left.location.path,
            left.location.start_byte,
            left.location.end_byte,
        )
            .cmp(&(
                &right.location.path,
                right.location.start_byte,
                right.location.end_byte,
            ))
    });

    let symbol_ids = delta
        .symbols
        .iter()
        .map(|value| value.id.as_str())
        .collect::<HashSet<_>>();
    let removed_symbol_ids = delta
        .removed_symbol_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let symbols = Arc::make_mut(&mut table.symbols);
    symbols.retain(|value| {
        !symbol_ids.contains(value.id.as_str()) && !removed_symbol_ids.contains(value.id.as_str())
    });
    symbols.extend(delta.symbols.iter().cloned());
    symbols.sort_by(|left, right| left.id.cmp(&right.id));
    for replacement in &delta.symbol_reference_files {
        if replacement
            .references
            .iter()
            .any(|reference| reference.path != replacement.path)
        {
            return Err(SessionError::InvalidResponse(format!(
                "reference delta for {:?} contains another path",
                replacement.path
            )));
        }
        let symbol_index = symbols
            .binary_search_by(|symbol| symbol.id.cmp(&replacement.id))
            .map_err(|_| {
                SessionError::InvalidResponse(format!(
                    "reference delta names missing symbol {:?}",
                    replacement.id
                ))
            })?;
        let symbol = &mut symbols[symbol_index];
        let start = symbol
            .references
            .partition_point(|reference| reference.path < replacement.path);
        let end = symbol
            .references
            .partition_point(|reference| reference.path <= replacement.path);
        symbol
            .references
            .splice(start..end, replacement.references.iter().cloned());
    }

    let file_paths = delta
        .files
        .iter()
        .map(|value| value.path.as_str())
        .collect::<HashSet<_>>();
    let removed_file_paths = delta
        .removed_file_paths
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let files = Arc::make_mut(&mut table.files);
    files.retain(|value| {
        !file_paths.contains(value.path.as_str())
            && !removed_file_paths.contains(value.path.as_str())
    });
    files.extend(delta.files.iter().cloned());
    files.sort_by(|left, right| left.path.cmp(&right.path));
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
