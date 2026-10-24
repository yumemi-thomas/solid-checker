//! One retained Type Facts lifecycle (Go: `session.go`): project identity,
//! generation, request validation and the exact error codes.
//!
//! The session owns the retained demand store, the state token and the table
//! that token names; the [`DemandClosure`] owns the rest of the analysis
//! (retained contributions, source digests, the transport manifest and the
//! table), asking the backend only compiler questions. `analyze` edits the
//! demand set in a transaction that rolls back unless every later step
//! succeeds, then picks a full, delta, or sparse transition, writes it to the
//! response or the arena, and only then publishes the new token.

use crate::backend::Backend;
use crate::closure::DemandClosure;
use crate::context::Context;
use crate::demands::RetainedDemandStore;
use crate::gocompat::{clean, quote};
use crate::trace::SharedTrace;
use crate::wire::{Incoming, blank_response, error_response};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use typefacts::SymbolFact;
use typefacts::producer::{
    EncodeError, EncodedTransition, ProducerTable, TransitionEncoder, TransitionInput,
    TransitionMode, TransitionSink,
};
use typefacts::v3::{
    Operation, Request, Response, ServerTimings, SourceFile, TYPE_FACTS_SCHEMA_V1,
};

const ERR_GENERATION_MISMATCH: &str = "type facts generation mismatch";
const ERR_SESSION_CLOSED: &str = "Type Facts session is closed";

#[derive(Default)]
struct Retained {
    token: u64,
    token_text: String,
    demands: RetainedDemandStore,
    /// What the last `analyze` handed the client: the table that `token_text`
    /// names, stripped to the identity the next transition is authenticated
    /// against.
    table: Option<ProducerTable>,
}

pub struct Session {
    closure: DemandClosure,
    project_id: String,
    schema: u64,
    retained: Retained,
    transition: TransitionEncoder,
    trace: SharedTrace,
    source_arena_path: Option<PathBuf>,
    closed: bool,
    close_error: Option<String>,
    /// Answer `sources` with each file's bytes in its descriptor rather than
    /// through a temporary arena file.
    inline_sources: bool,
}

impl Session {
    /// Takes ownership of `backend`, including when construction fails.
    pub fn new(
        backend: Box<dyn Backend>,
        project_id: &str,
        inline_sources: bool,
    ) -> Result<Self, String> {
        let project_id = clean(project_id);
        if project_id.is_empty() || project_id == "." {
            let mut backend = backend;
            let _ = backend.close();
            return Err("Type Facts session requires a project identity".to_owned());
        }
        Ok(Self {
            closure: DemandClosure::new(backend, None),
            project_id,
            schema: TYPE_FACTS_SCHEMA_V1,
            retained: Retained::default(),
            transition: TransitionEncoder::default(),
            trace: None,
            source_arena_path: None,
            closed: false,
            close_error: None,
            inline_sources,
        })
    }

    /// Reports producer-side stages (`analyze-transport-*`) to `trace`.
    #[must_use]
    pub fn with_trace(mut self, trace: SharedTrace) -> Self {
        self.closure.trace.clone_from(&trace);
        self.trace = trace;
        self
    }

    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.closure.generation()
    }

    /// The demand closure, for tests that inspect what it retains.
    pub fn closure(&self) -> &DemandClosure {
        &self.closure
    }

    /// The closure's backend, for tests that inspect their double.
    pub fn backend_mut(&mut self) -> &mut dyn Backend {
        self.closure.backend_mut()
    }

    /// The demand set the session retains.
    pub fn demands(&mut self) -> &mut RetainedDemandStore {
        &mut self.retained.demands
    }

    /// The table the last accepted `analyze` was published as, and its token.
    pub fn published(&self) -> (Option<&ProducerTable>, &str) {
        (self.retained.table.as_ref(), &self.retained.token_text)
    }

    /// Go's `ValidateLifecycleRequest`.
    fn validate(&self, incoming: &Incoming) -> Result<(), String> {
        let request = &incoming.request;
        if request.schema != TYPE_FACTS_SCHEMA_V1 {
            return Err(format!("unsupported TypeFacts schema {}", request.schema));
        }
        if request.request_id == 0 || request.project_id.is_empty() || request.generation == 0 {
            return Err(ERR_GENERATION_MISMATCH.to_owned());
        }
        if let Some(operation) = &incoming.bad_operation {
            return Err(format!(
                "unsupported lifecycle operation {}",
                quote(operation)
            ));
        }
        Ok(())
    }

    /// Answers one request; every request yields exactly one response.
    pub fn lifecycle(&mut self, ctx: &Context, incoming: &Incoming) -> Response {
        self.lifecycle_into(ctx, incoming, None)
    }

    /// As [`Self::lifecycle`], writing a packed table transition into `sink`
    /// (the transition arena) instead of the response when there is one.
    /// Lifecycle metadata stays in the ordinary response.
    pub fn lifecycle_into(
        &mut self,
        ctx: &Context,
        incoming: &Incoming,
        sink: Option<&mut dyn TransitionSink>,
    ) -> Response {
        let request = &incoming.request;
        let mut response = blank_response(
            self.schema,
            request.request_id,
            &self.project_id,
            self.closure.generation(),
        );
        if let Err(message) = self.validate(incoming) {
            return error_response(response, "invalid-request", message);
        }
        if clean(&request.project_id) != self.project_id {
            return error_response(response, "project-mismatch", ERR_GENERATION_MISMATCH);
        }
        if self.closed {
            if request.operation == Operation::Close && self.close_error.is_none() {
                response.ok = true;
                return response;
            }
            return error_response(response, "session-closed", ERR_SESSION_CLOSED);
        }
        let generation = self.closure.generation();
        match request.operation {
            Operation::Open => {
                if request.generation != generation {
                    return error_response(
                        response,
                        "generation-mismatch",
                        ERR_GENERATION_MISMATCH,
                    );
                }
            }
            Operation::Update => {
                if request.generation != generation + 1 {
                    return error_response(
                        response,
                        "generation-mismatch",
                        ERR_GENERATION_MISMATCH,
                    );
                }
                match self.closure.update(ctx, &request.changes) {
                    Ok(outcome) => {
                        response.generation = self.closure.generation();
                        response.affected = outcome.affected;
                    }
                    Err(error) => return error_response(response, "update-failed", error.0),
                }
            }
            Operation::Analyze => {
                if request.generation != generation {
                    return error_response(
                        response,
                        "generation-mismatch",
                        ERR_GENERATION_MISMATCH,
                    );
                }
                if request.compact_demands.is_some() && !request.demands.is_empty() {
                    return error_response(
                        response,
                        "invalid-demands",
                        "analyze carries both demands and compactDemands",
                    );
                }
                // Analyze is always retained-state scoped: a caller either
                // resets the state or presents the token the previous analyze
                // handed back.
                if !request.reset_state && request.state_token != self.retained.token_text {
                    return error_response(response, "state-mismatch", ERR_GENERATION_MISMATCH);
                }
                if !request.reset_state
                    && request.demands.is_empty()
                    && request
                        .compact_demands
                        .as_ref()
                        .is_none_or(|compact| compact.groups.is_empty())
                    && request.removed_demand_paths.is_empty()
                    && self
                        .retained
                        .table
                        .as_ref()
                        .is_some_and(|table| table.generation == generation)
                {
                    response.state_token = self.retained.token_text.clone();
                    response.timings = Some(ServerTimings::default());
                    response.ok = true;
                    return response;
                }
                return self.analyze(ctx, request, response, sink);
            }
            Operation::Symbols => {
                if request.generation != generation {
                    return error_response(
                        response,
                        "generation-mismatch",
                        ERR_GENERATION_MISMATCH,
                    );
                }
                if request.state_token.is_empty() || request.state_token != self.retained.token_text
                {
                    return error_response(
                        response,
                        "state-mismatch",
                        format!(
                            "{ERR_GENERATION_MISMATCH}: symbol evidence token {}, retained token {}",
                            quote(&request.state_token),
                            quote(&self.retained.token_text)
                        ),
                    );
                }
                if let Err(message) = populate_symbol_evidence(
                    &mut self.closure,
                    &self.transition,
                    &self.project_id,
                    ctx,
                    request,
                    &mut response,
                    sink,
                ) {
                    if let Some(reason) = ctx.err() {
                        return error_response(response, "analysis-cancelled", reason);
                    }
                    return error_response(response, "analysis-failed", message);
                }
            }
            Operation::Sources => {
                if request.generation != generation {
                    return error_response(
                        response,
                        "generation-mismatch",
                        ERR_GENERATION_MISMATCH,
                    );
                }
                let sources = match self.closure.source_files(ctx) {
                    Ok(sources) => sources,
                    Err(error) => return error_response(response, "sources-failed", error.0),
                };
                if self.inline_sources {
                    response.sources = sources
                        .into_iter()
                        .map(|source| SourceFile {
                            path: source.path,
                            local: false,
                            source: source.source,
                        })
                        .collect();
                } else {
                    match self.write_source_arena(&sources) {
                        Ok((path, descriptors, lengths)) => {
                            response.source_arena = path;
                            response.sources = descriptors;
                            response.source_lengths = lengths;
                        }
                        Err(message) => {
                            return error_response(response, "sources-failed", message);
                        }
                    }
                }
            }
            Operation::Modules => {
                if request.generation != generation {
                    return error_response(
                        response,
                        "generation-mismatch",
                        ERR_GENERATION_MISMATCH,
                    );
                }
                let demand = request.module_graph.clone().unwrap_or_default();
                match self.closure.module_graph(ctx, &demand) {
                    Ok(inventory) => {
                        response.modules = inventory.modules;
                        response.module_imports = inventory.imports;
                        response.unknown_import_paths = inventory.unknown_import_paths;
                        response.unmapped_roots = inventory.unmapped_roots;
                    }
                    Err(error) => {
                        if let Some(reason) = ctx.err() {
                            return error_response(response, "analysis-cancelled", reason);
                        }
                        return error_response(response, "modules-failed", error.0);
                    }
                }
            }
            Operation::Cancel => {
                // Cancellation is delivered through the active request's
                // context by the transport; this operation acknowledges it.
            }
            Operation::Close => {
                if request.generation != generation {
                    return error_response(
                        response,
                        "generation-mismatch",
                        ERR_GENERATION_MISMATCH,
                    );
                }
                if let Err(message) = self.close() {
                    return error_response(response, "close-failed", message);
                }
                response.ok = true;
                return response;
            }
        }
        response.ok = true;
        response
    }

    /// The tail of `analyze`: edit the demand set, build the table, pick the
    /// transition, publish the new retained state. Nothing is published unless
    /// every step succeeds: the demand transaction rolls back when dropped
    /// uncommitted, and a failure after the closure began materializing
    /// abandons its derived caches.
    #[allow(clippy::too_many_lines)]
    fn analyze(
        &mut self,
        ctx: &Context,
        request: &Request,
        mut response: Response,
        mut sink: Option<&mut dyn TransitionSink>,
    ) -> Response {
        let Self {
            closure,
            project_id,
            retained,
            transition: encoder,
            trace,
            ..
        } = self;
        let Retained {
            token,
            token_text,
            demands,
            table,
        } = retained;
        let generation = closure.generation();
        let mut transaction = if let Some(compact) = &request.compact_demands {
            match demands.begin_compact(compact, &request.removed_demand_paths, request.reset_state)
            {
                Ok(transaction) => transaction,
                Err(message) => return error_response(response, "invalid-demands", message),
            }
        } else {
            demands.begin(
                &request.demands,
                &request.removed_demand_paths,
                request.reset_state,
            )
        };
        let started = Instant::now();
        let build_sequence = closure.stats().build_sequence;
        // From here a table that fails to publish drops every derived cache
        // (Go: the deferred `abandonAnalysis`); the transaction rolls back
        // when it is dropped.
        macro_rules! bail {
            ($code:expr, $message:expr) => {{
                closure.abandon_analysis();
                return error_response(response, $code, $message);
            }};
        }
        let analyzed = {
            let (groups, paths) = transaction.parts();
            closure.demand_table_for_canonical_groups(ctx, generation, groups, paths)
        };
        let mut analyzed = match analyzed {
            Ok(analyzed) => analyzed,
            Err(error) => {
                if let Some(reason) = ctx.err() {
                    bail!("analysis-cancelled", reason);
                }
                bail!("analysis-failed", error.0);
            }
        };
        if let Some(reason) = ctx.err() {
            bail!("analysis-cancelled", reason);
        }
        let stats = closure.stats();
        let mut timings = ServerTimings {
            analyze_ns: nanos(started.elapsed()),
            materialized: stats.build_sequence != build_sequence,
            ..ServerTimings::default()
        };
        if timings.materialized {
            timings.async_ns = nanos(stats.async_duration);
            timings.demand_ns = nanos(stats.demand_duration);
            timings.assembly_ns = nanos(stats.assembly_duration);
            timings.sort_ns = nanos(stats.sort_duration);
            timings.close_symbols_ns = nanos(stats.close_duration);
            timings.retained_files = stats.retention.retained_files as u64;
            timings.recomputed_files = stats.retention.recomputed_files as u64;
            timings.non_durable_files = stats.retention.non_durable_files as u64;
        }
        response.timings = Some(timings);
        let next_token = *token + 1;
        let next_token_text = next_token.to_string();
        response.state_token.clone_from(&next_token_text);

        // Building the wire form is not part of the analysis the response
        // reports, so it is traced separately. Without this the cost shows up
        // nowhere and reads as client or transport overhead.
        let transport_started = Instant::now();
        let base = if request.reset_state {
            None
        } else {
            table.as_ref()
        };
        let base_token = if base.is_some() {
            token_text.as_str()
        } else {
            ""
        };
        let mut encoded = encode_transition(
            encoder,
            TransitionInput {
                project_id,
                base_state_token: base_token,
                base,
                target: &analyzed,
                sparse: true,
            },
            reborrow(&mut sink),
        );
        if matches!(encoded, Err(EncodeError::SparseUnavailable)) {
            // Exact reference evidence can be unavailable after broad compiler
            // invalidation. Rebuild once and send a full replacement; the
            // normal sparse edit path stays proportional to changed files.
            closure.force_full_materialization();
            let rebuilt = {
                let (groups, paths) = transaction.parts();
                closure.demand_table_for_canonical_groups(ctx, generation, groups, paths)
            };
            encoded = match rebuilt {
                Ok(rebuilt) => {
                    analyzed = rebuilt;
                    encode_transition(
                        encoder,
                        TransitionInput {
                            project_id,
                            base_state_token: "",
                            base: None,
                            target: &analyzed,
                            sparse: false,
                        },
                        reborrow(&mut sink),
                    )
                }
                Err(error) => Err(EncodeError::Invalid(error.0)),
            };
        }
        let transition = match encoded {
            Ok(transition) => transition,
            Err(error) => bail!("assembly-failed", error.to_string()),
        };
        let mut mode = transition.mode.as_str();
        // Within one source generation an empty delta is genuine reuse and
        // needs no frame. Across generations the empty delta header still
        // advances the retained table identity.
        let retained_generation = table.as_ref().map(|retained| retained.generation);
        if transition.mode != TransitionMode::Delta
            || Some(analyzed.generation) != retained_generation
            || transition.path_operations != 0
            || transition.symbol_operations != 0
        {
            response.table_transition = transition.bytes;
        } else {
            mode = "reuse";
        }
        if let Some(trace) = trace {
            trace.stage(
                &format!("analyze-transport-{mode}"),
                transport_started.elapsed(),
            );
        }
        if let Some(reason) = ctx.err() {
            bail!("analysis-cancelled", reason);
        }
        let wants_evidence = !request.symbol_queries.is_empty()
            || request.reference_changes
            || request.release_analysis;
        if wants_evidence
            && let Err(message) = populate_symbol_evidence(
                closure,
                encoder,
                project_id,
                ctx,
                request,
                &mut response,
                None,
            )
        {
            if let Some(reason) = ctx.err() {
                bail!("analysis-cancelled", reason);
            }
            bail!("analysis-failed", message);
        }
        if let Some(reason) = ctx.err() {
            bail!("analysis-cancelled", reason);
        }
        *token = next_token;
        *token_text = next_token_text;
        transaction.commit();
        // The retained table is an intentionally detached header that
        // authenticates the next transition. Its expanded rows now belong to
        // the client; keeping them would pin them for the session.
        *table = Some(ProducerTable {
            sources: Vec::new(),
            entities: typefacts::producer::EntityRows::default(),
            files: Vec::new(),
            ..analyzed
        });
        closure.release_transport_rows();
        response.ok = true;
        response
    }

    /// Closes the backend and removes the sources arena. Idempotent: later
    /// calls return the first outcome.
    pub fn close(&mut self) -> Result<(), String> {
        if self.closed {
            return self.close_error.clone().map_or(Ok(()), Err);
        }
        self.closed = true;
        let mut failures = Vec::new();
        if let Err(error) = self.closure.close() {
            failures.push(error.0);
        }
        if let Err(message) = remove_source_arena(self.source_arena_path.take()) {
            failures.push(message);
        }
        if failures.is_empty() {
            Ok(())
        } else {
            let message = failures.join("\n");
            self.close_error = Some(message.clone());
            Err(message)
        }
    }

    #[allow(clippy::type_complexity)]
    fn write_source_arena(
        &mut self,
        sources: &[SourceFile],
    ) -> Result<(String, Vec<SourceFile>, Vec<u64>), String> {
        use std::io::Write;
        remove_source_arena(self.source_arena_path.take())?;
        let (path, file) = create_temp_file().map_err(|error| error.to_string())?;
        let mut writer = std::io::BufWriter::with_capacity(1 << 20, file);
        let mut descriptors = Vec::with_capacity(sources.len());
        let mut lengths = Vec::with_capacity(sources.len());
        let outcome = (|| -> std::io::Result<()> {
            for source in sources {
                writer.write_all(&source.source)?;
                descriptors.push(SourceFile {
                    path: source.path.clone(),
                    local: false,
                    source: Vec::new(),
                });
                lengths.push(source.source.len() as u64);
            }
            writer.flush()
        })();
        if let Err(error) = outcome {
            let _ = std::fs::remove_file(&path);
            return Err(error.to_string());
        }
        self.source_arena_path = Some(path.clone());
        Ok((path.to_string_lossy().into_owned(), descriptors, lengths))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Go's `populateSymbolEvidence`: evidence for the request's queries, a
/// changed-reference census, and the post-analysis release.
fn populate_symbol_evidence(
    closure: &mut DemandClosure,
    encoder: &TransitionEncoder,
    project_id: &str,
    ctx: &Context,
    request: &Request,
    response: &mut Response,
    sink: Option<&mut dyn TransitionSink>,
) -> Result<(), String> {
    let mut evidence = closure
        .resolve_symbol_evidence(ctx, &request.symbol_queries)
        .map_err(|error| error.0)?;
    if request.operation == Operation::Symbols && !evidence.is_empty() {
        evidence.sort_by(|left, right| left.id.cmp(&right.id));
        // A packed table holds each symbol once, in strictly ascending id
        // order; a request naming an id twice still gets a decodable table.
        evidence.dedup_by(|later, earlier| later.id == earlier.id);
        let mut target = ProducerTable::new(request.generation, project_id);
        target.symbols = evidence;
        let packed = encode_transition(
            encoder,
            TransitionInput {
                project_id,
                base_state_token: "",
                base: None,
                target: &target,
                sparse: false,
            },
            sink,
        )
        .map_err(|error| format!("pack symbol evidence: {error}"))?;
        response.table_transition = packed.bytes;
    } else {
        response.symbol_evidence = evidence;
    }
    if request.reference_changes {
        let (ids, exact) = closure.changed_references(ctx).map_err(|error| error.0)?;
        response.changed_reference_symbols.clone_from(&ids);
        response.reference_changes_exact = exact;
        // The evidence rides along only for a client that names the paths
        // it patches; one that closes its own symbols asks for what it holds.
        if exact && !ids.is_empty() && !request.reference_paths.is_empty() {
            let queries: Vec<_> = ids
                .into_iter()
                .map(|id| typefacts::v3::SymbolQuery {
                    id,
                    references: true,
                    references_only: false,
                })
                .collect();
            let mut evidence = closure
                .resolve_symbol_evidence(ctx, &queries)
                .map_err(|error| error.0)?;
            if !request.reference_paths.is_empty() {
                let paths: HashSet<String> = request
                    .reference_paths
                    .iter()
                    .map(|path| clean(path))
                    .collect();
                for fact in &mut evidence {
                    let kept: Vec<_> = fact
                        .references
                        .iter()
                        .filter(|location| paths.contains(location.path.as_ref()))
                        .cloned()
                        .collect();
                    *fact = SymbolFact {
                        references: kept.into(),
                        ..fact.clone()
                    };
                }
            }
            response.reference_evidence = evidence;
        }
    }
    // An inexact reference delta leaves the client to ask for the references
    // it needs next, in the same generation: its request carries the release.
    if request.release_analysis && !(request.reference_changes && !response.reference_changes_exact)
    {
        closure.release_backend_analysis_state();
    }
    Ok(())
}

/// A shorter borrow of the optional sink. The explicit cast shortens the
/// trait object's own lifetime too, which `&mut` would otherwise pin.
fn reborrow<'a>(
    sink: &'a mut Option<&mut dyn TransitionSink>,
) -> Option<&'a mut dyn TransitionSink> {
    sink.as_mut()
        .map(|sink| &mut **sink as &mut dyn TransitionSink)
}

fn nanos(elapsed: std::time::Duration) -> u64 {
    u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX)
}

/// Streams into `sink` when there is one; either way the caller sees the same
/// counts and mode.
fn encode_transition(
    encoder: &TransitionEncoder,
    input: TransitionInput<'_>,
    sink: Option<&mut dyn TransitionSink>,
) -> Result<EncodedTransition, EncodeError> {
    match sink {
        Some(sink) => encoder.encode_into(input, sink),
        None => encoder.encode(input),
    }
}

fn remove_source_arena(path: Option<PathBuf>) -> Result<(), String> {
    let Some(path) = path else { return Ok(()) };
    match std::fs::remove_file(&path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(format!(
            "remove {}: {}",
            path.display(),
            crate::gocompat::io_message(&error)
        )),
        _ => Ok(()),
    }
}

/// `os.CreateTemp("", "solid-typefacts-sources-*")`: a new 0600 file in the
/// temporary directory.
fn create_temp_file() -> std::io::Result<(PathBuf, std::fs::File)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = std::env::temp_dir();
    let mut attempts = 0;
    loop {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos());
        let suffix = u64::from(std::process::id())
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(u64::from(nanos))
            .wrapping_add(NEXT.fetch_add(1, Ordering::Relaxed));
        let path = directory.join(format!("solid-typefacts-sources-{suffix}"));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error)
                if error.kind() == std::io::ErrorKind::AlreadyExists && attempts < 10_000 =>
            {
                attempts += 1;
            }
            Err(error) => return Err(error),
        }
    }
}
