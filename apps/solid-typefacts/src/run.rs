//! Process orchestration (main.go `run`): flags, startup handshake, project
//! open, then the serve loop. Order matters and is the Go order: parse and
//! validate flags, profile file, absolute project path, transition arena,
//! handshake written and flushed before anything is opened, then the config
//! overlay and the backend.

use crate::arena::{self, TransitionArena};
use crate::args::{self, SCHEMA_VERSION_V1};
use crate::backend::OpenBackend;
use crate::context::Context;
use crate::frame::write_frame;
use crate::gocompat::{absolute, path_error};
use crate::serve::{Responder, serve};
use crate::session::Session;
use crate::trace::{self, SharedTrace};
use crate::wire::{Incoming, handshake_payload};
use std::io::{BufWriter, Read, Write};
use std::time::Instant;
use typefacts::v3::{Operation, Response};

/// Runs the producer on the process's stdin and stdout over the tsrs backend.
pub fn run_stdio(args: &[String]) -> Result<(), String> {
    let trace = trace::from_environment();
    run(
        args,
        std::io::BufReader::new(std::io::stdin()),
        std::io::stdout(),
        crate::project::opener(trace.clone()),
        trace,
        &CrashMarkers::from_environment(),
    )
}

/// Test-only fault injection: marker file paths resolved once at startup so
/// no environment lookup sits on the per-request path.
#[derive(Clone, Debug, Default)]
pub struct CrashMarkers {
    pub before_update: String,
    pub before_analyze: String,
    /// As the crash markers, but the worker thread panics instead of exiting.
    pub panic_before_update: String,
    pub panic_before_analyze: String,
}

impl CrashMarkers {
    #[must_use]
    pub fn from_environment() -> Self {
        let read = |name: &str| {
            std::env::var_os(name)
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        Self {
            before_update: read("SOLID_TYPEFACTS_CRASH_BEFORE_UPDATE"),
            before_analyze: read("SOLID_TYPEFACTS_CRASH_BEFORE_ANALYZE"),
            panic_before_update: read("SOLID_TYPEFACTS_PANIC_BEFORE_UPDATE"),
            panic_before_analyze: read("SOLID_TYPEFACTS_PANIC_BEFORE_ANALYZE"),
        }
    }
}

pub fn run(
    args: &[String],
    input: impl Read + Send + 'static,
    output: impl Write + Send + 'static,
    open_backend: OpenBackend,
    trace: SharedTrace,
    crash: &CrashMarkers,
) -> Result<(), String> {
    let started = Instant::now();
    let options = args::parse(args)?;
    if options.project.is_empty() {
        return Err("-project is required".to_owned());
    }
    if options.schema != SCHEMA_VERSION_V1 {
        return Err(format!("unsupported TypeFacts schema {}", options.schema));
    }
    // Go would write a CPU profile here. There is none to write; creating the
    // file keeps the flag's failure mode and leaves an empty profile behind.
    if !options.cpu_profile.is_empty() {
        std::fs::File::create(&options.cpu_profile).map_err(|error| {
            format!(
                "create cpu profile: {}",
                path_error("open", &options.cpu_profile, &error)
            )
        })?;
    }
    let project_id =
        absolute(&options.project).map_err(|error| format!("resolve project: {error}"))?;
    let arena = arena::open(&options.transition_arena)?;

    // The handshake carries nothing derived from the project, so it goes out
    // before the program is opened: a client blocked on it is released as
    // soon as the process is live.
    let mut writer = BufWriter::new(output);
    let handshake =
        handshake_payload().map_err(|error| format!("encode startup handshake: {error}"))?;
    write_frame(&mut writer, &handshake)
        .map_err(|error| format!("write startup handshake: {error}"))?;
    writer.flush().map_err(|error| {
        format!(
            "flush startup handshake: {}",
            crate::gocompat::io_message(&error)
        )
    })?;
    if let Some(trace) = &trace {
        trace.stage("handshake-written", started.elapsed());
    }

    let config_source = if options.project_config.is_empty() {
        None
    } else {
        let bytes = std::fs::read(&options.project_config).map_err(|error| {
            format!(
                "read -project-config: {}",
                path_error("open", &options.project_config, &error)
            )
        })?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    };

    let make = {
        let project_id = project_id.clone();
        let trace = trace.clone();
        let crash = crash.clone();
        let inline_sources = options.inline_sources;
        move || -> Result<LifecycleResponder, String> {
            let ctx = Context::new();
            let backend = open_backend(&ctx, &project_id, config_source.as_deref())
                .map_err(|error| format!("open TypeScript project: {error}"))?;
            let session =
                Session::new(backend, &project_id, inline_sources)?.with_trace(trace.clone());
            Ok(LifecycleResponder {
                session,
                trace,
                transition_arena: arena,
                crash,
            })
        }
    };
    let opened_trace = trace.clone();
    serve(
        make,
        move || {
            if let Some(trace) = &opened_trace {
                trace.stage("open", started.elapsed());
            }
        },
        input,
        writer,
        &trace,
    )
}

pub struct LifecycleResponder {
    session: Session,
    trace: SharedTrace,
    /// Held open for the lifetime of the process. `analyze` and `symbols`
    /// write their packed transition into it instead of the response.
    transition_arena: Option<TransitionArena>,
    crash: CrashMarkers,
}

impl Responder for LifecycleResponder {
    fn lifecycle(&mut self, ctx: &Context, incoming: &Incoming) -> Response {
        match incoming.operation() {
            Some(Operation::Update) => {
                crash_on_marker(&self.crash.before_update);
                panic_on_marker(&self.crash.panic_before_update);
            }
            Some(Operation::Analyze) => {
                crash_on_marker(&self.crash.before_analyze);
                panic_on_marker(&self.crash.panic_before_analyze);
            }
            _ => {}
        }
        let started = Instant::now();
        let response = self.answer(ctx, incoming);
        if let Some(trace) = &self.trace {
            if incoming.operation() == Some(Operation::Update) {
                trace.stage("update", started.elapsed());
            } else if let Some(timings) = &response.timings {
                trace.stage(
                    "analyze",
                    std::time::Duration::from_nanos(timings.analyze_ns),
                );
            }
        }
        response
    }
}

impl LifecycleResponder {
    /// Go's `lifecycleResponder.lifecycle` arena branch: begin the request's
    /// transition, let the session stream into it, and point the client at it.
    /// If the arena cannot begin, the request is answered inline instead.
    fn answer(&mut self, ctx: &Context, incoming: &Incoming) -> Response {
        let streams = matches!(
            incoming.operation(),
            Some(Operation::Analyze | Operation::Symbols)
        );
        let Some(arena) = self.transition_arena.as_mut().filter(|_| streams) else {
            return self.session.lifecycle(ctx, incoming);
        };
        if arena.begin(incoming.request.request_id).is_err() {
            return self.session.lifecycle(ctx, incoming);
        }
        let mut response = self.session.lifecycle_into(ctx, incoming, Some(arena));
        if let Some((path, descriptor)) = arena.descriptor()
            && response.ok
        {
            response.source_arena = path;
            response.source_lengths = descriptor;
        }
        response
    }
}

/// Terminates the process when `path` names an existing marker file,
/// consuming the marker so a restarted service runs normally.
fn crash_on_marker(path: &str) {
    if !path.is_empty() && std::fs::remove_file(path).is_ok() {
        std::process::exit(1);
    }
}

/// Panics the calling (worker) thread when `path` names an existing marker
/// file, consuming the marker.
fn panic_on_marker(path: &str) {
    if !path.is_empty() && std::fs::remove_file(path).is_ok() {
        panic!("injected worker panic");
    }
}
