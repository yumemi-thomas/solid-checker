//! One retained Type Facts lifecycle: project identity, generation, request
//! validation and the exact error codes of internal/typefacts/session.go.
//!
//! The demand closure behind `analyze` and the symbol store behind `symbols`
//! are S2; here those operations run Go's pre-checks (generation, token,
//! demand shape) and then report that the closure is not built yet. No real
//! backend can be opened in S1, so that answer is reachable only through a
//! test backend.

use crate::backend::Backend;
use crate::context::Context;
use crate::gocompat::{clean, quote};
use crate::wire::{Incoming, blank_response, error_response};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use typefacts::v3::{Operation, Response, SourceFile, TYPE_FACTS_SCHEMA_V1};

const ERR_GENERATION_MISMATCH: &str = "type facts generation mismatch";
const ERR_SESSION_CLOSED: &str = "Type Facts session is closed";

pub struct Session {
    backend: Box<dyn Backend>,
    project_id: String,
    generation: u64,
    schema: u64,
    /// The token the last `analyze` handed back; empty before the first.
    retained_token_text: String,
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
            backend,
            project_id,
            generation: 1,
            schema: TYPE_FACTS_SCHEMA_V1,
            retained_token_text: String::new(),
            source_arena_path: None,
            closed: false,
            close_error: None,
            inline_sources,
        })
    }

    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
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
        let request = &incoming.request;
        let mut response = blank_response(
            self.schema,
            request.request_id,
            &self.project_id,
            self.generation,
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
        let generation = self.generation;
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
                match self.backend.update(ctx, &request.changes) {
                    Ok(outcome) => {
                        self.generation = generation + 1;
                        response.generation = self.generation;
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
                if !request.reset_state && request.state_token != self.retained_token_text {
                    return error_response(response, "state-mismatch", ERR_GENERATION_MISMATCH);
                }
                return error_response(
                    response,
                    "analysis-failed",
                    "analyze is not implemented in this build (migration slice S2)",
                );
            }
            Operation::Symbols => {
                if request.generation != generation {
                    return error_response(
                        response,
                        "generation-mismatch",
                        ERR_GENERATION_MISMATCH,
                    );
                }
                if request.state_token.is_empty() || request.state_token != self.retained_token_text
                {
                    return error_response(
                        response,
                        "state-mismatch",
                        format!(
                            "{ERR_GENERATION_MISMATCH}: symbol evidence token {}, retained token {}",
                            quote(&request.state_token),
                            quote(&self.retained_token_text)
                        ),
                    );
                }
                return error_response(
                    response,
                    "analysis-failed",
                    "symbols is not implemented in this build (migration slice S2)",
                );
            }
            Operation::Sources => {
                if request.generation != generation {
                    return error_response(
                        response,
                        "generation-mismatch",
                        ERR_GENERATION_MISMATCH,
                    );
                }
                let sources = match self.backend.source_files(ctx) {
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
                match self.backend.module_graph(ctx, &demand) {
                    Ok(inventory) => {
                        response.modules = inventory.modules;
                        response.module_imports = inventory.imports;
                        response.unknown_import_paths = inventory.unknown_import_paths;
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

    /// Closes the backend and removes the sources arena. Idempotent: later
    /// calls return the first outcome.
    pub fn close(&mut self) -> Result<(), String> {
        if self.closed {
            return self.close_error.clone().map_or(Ok(()), Err);
        }
        self.closed = true;
        let mut failures = Vec::new();
        if let Err(error) = self.backend.close() {
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
