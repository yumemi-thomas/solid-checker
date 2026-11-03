//! The differential fuzzer's executor: the work of a grid that is not judgement, off the Node orchestrator's one thread.
//!
//! The JS pipeline (scripts/fuzz/exec.mjs) generates programs and judges them; this process writes their projects, type-checks them
//! (`tsrs -b` batches), runs the checker, and runs the fast path's page
//! (chrome-host/runner.tsx) in Chrome over the DevTools protocol, with the browser switches and page setup Playwright uses (less its
//! two observing domains, `open_page`)
//! (chrome-host.mjs is the reference: its records and this executor's must be identical). A Node helper (vite-host.mjs) serves the
//! pages with Vite and the Solid plugin, as the oracle compiles them, and maps a record's stack positions through Vite's source maps.
//!
//! Protocol: one JSON object per line on stdin, one per line on stdout, matched by `seq`.
//!   init     the tools, the concurrency, (optional) the Chrome settings and `warmChecker` (off, on, verify); answers `ready`
//!   prepare  the case's files, its clean and twin projects (in a fixed slot pair, or given directories), the type checks and the
//!            checker run; answers the type checks and the checker's raw output
//!   run      cases to run in Chrome; answers their records by id
//!   checks   how many checker runs may run at once from now on (at most the step-1 pool); answers the new number
//! Parsing the checker's output and every verdict stay in JS: this process returns what the tools printed.

use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

mod typecheck;

use typecheck::{typecheck_batch, typecheck_one};

type Res<T> = Result<T, String>;

// ---------------------------------------------------------------------------------------------------------------------------------
// Small tools

struct Out(Mutex<std::io::Stdout>);
impl Out {
    fn emit(&self, value: &Value) {
        let mut out = self.0.lock().unwrap();
        let _ = serde_json::to_writer(&mut *out, value);
        let _ = out.write_all(b"\n");
        let _ = out.flush();
    }
}

/// A counting semaphore whose capacity can change while it is held (`resize`): `free` goes below zero when it shrinks under its
/// holders, and acquiring waits until releases bring it back above.
struct Semaphore {
    free: Mutex<isize>,
    capacity: Mutex<usize>,
    ready: Condvar,
}
impl Semaphore {
    fn new(count: usize) -> Self {
        Self { free: Mutex::new(count.max(1) as isize), capacity: Mutex::new(count.max(1)), ready: Condvar::new() }
    }
    fn acquire(&self) {
        let mut free = self.free.lock().unwrap();
        while *free <= 0 {
            free = self.ready.wait(free).unwrap();
        }
        *free -= 1;
    }
    fn release(&self) {
        *self.free.lock().unwrap() += 1;
        self.ready.notify_one();
    }
    /// The semaphore's capacity, now `count` (at least 1).
    fn resize(&self, count: usize) {
        let count = count.max(1);
        let mut capacity = self.capacity.lock().unwrap();
        *self.free.lock().unwrap() += count as isize - *capacity as isize;
        *capacity = count;
        self.ready.notify_all();
    }
    fn capacity(&self) -> usize {
        *self.capacity.lock().unwrap()
    }
}

/// Held around every process start. On macOS `std` creates a child's pipes and marks them close-on-exec in two steps, so a child
/// another thread starts in between inherits them: a retained checker daemon (`--warm-checker`) that a client started then holds
/// another command's stdout open for minutes, and that command's reader never sees its end (a grid stalled on one `prepare`).
static SPAWN: Mutex<()> = Mutex::new(());

/// `command.spawn()` with no other start in progress (`SPAWN`).
fn spawn(command: &mut Command) -> std::io::Result<std::process::Child> {
    let _held = SPAWN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    command.spawn()
}

struct Ran {
    status: Option<i32>,
    stdout: String,
    stderr: String,
    killed: bool,
    wall_ms: u128,
    spawn_error: Option<String>,
    /// The process's resource use as `wait4` gave it (Linux only; the harness's cost record, never a verdict input).
    usage: Option<Usage>,
}

/// A finished process's CPU time, peak resident set and block writes, including the children it waited for (Linux `wait4`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Usage {
    cpu_ms: u64,
    max_rss_kb: u64,
    write_kb: u64,
}

/// Waits for `child` as `Child::wait` does, with its resource use from `wait4`; the exit code is `ExitStatus::code`'s.
#[cfg(target_os = "linux")]
fn wait_with_usage(child: &mut std::process::Child) -> (Option<i32>, Option<Usage>) {
    #[repr(C)]
    struct Timeval {
        sec: i64,
        usec: i64,
    }
    #[repr(C)]
    struct Rusage {
        utime: Timeval,
        stime: Timeval,
        // maxrss, ixrss, idrss, isrss, minflt, majflt, nswap, inblock, oublock, msgsnd, msgrcv, nsignals, nvcsw, nivcsw
        longs: [i64; 14],
    }
    unsafe extern "C" {
        fn wait4(pid: i32, status: *mut i32, options: i32, rusage: *mut Rusage) -> i32;
    }
    let pid = child.id() as i32;
    let mut status = 0i32;
    let mut usage = Rusage { utime: Timeval { sec: 0, usec: 0 }, stime: Timeval { sec: 0, usec: 0 }, longs: [0; 14] };
    loop {
        // SAFETY: `pid` is this process's unreaped child; `status` and `usage` are valid for the call's writes.
        let reaped = unsafe { wait4(pid, &mut status, 0, &mut usage) };
        if reaped == pid {
            break;
        }
        if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return (child.wait().ok().and_then(|status| status.code()), None);
        }
    }
    let code = if status & 0x7f == 0 { Some((status >> 8) & 0xff) } else { None };
    let ms = |t: &Timeval| (t.sec.max(0) as u64) * 1000 + (t.usec.max(0) as u64) / 1000;
    let usage = Usage { cpu_ms: ms(&usage.utime) + ms(&usage.stime), max_rss_kb: usage.longs[0].max(0) as u64, write_kb: (usage.longs[8].max(0) as u64) / 2 };
    (code, Some(usage))
}

#[cfg(not(target_os = "linux"))]
fn wait_with_usage(child: &mut std::process::Child) -> (Option<i32>, Option<Usage>) {
    (child.wait().ok().and_then(|status| status.code()), None)
}

/// Runs a command to completion with its outputs read on threads, killed after `timeout`.
fn run_command(mut command: Command, timeout: Duration) -> Ran {
    let started = Instant::now();
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match spawn(&mut command) {
        Ok(child) => child,
        Err(error) => {
            return Ran { status: None, stdout: String::new(), stderr: String::new(), killed: false, wall_ms: 0, spawn_error: Some(error.to_string()), usage: None };
        }
    };
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || {
        let mut text = Vec::new();
        let _ = stdout.read_to_end(&mut text);
        text
    });
    let err = thread::spawn(move || {
        let mut text = Vec::new();
        let _ = stderr.read_to_end(&mut text);
        text
    });
    // A watchdog kills the command at its timeout; the wait itself blocks (no polling).
    let (finished, watch) = channel::<()>();
    let pid = child.id();
    let watchdog = thread::spawn(move || {
        if watch.recv_timeout(timeout) == Err(RecvTimeoutError::Timeout) {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
            return true;
        }
        false
    });
    let (status, usage) = wait_with_usage(&mut child);
    let _ = finished.send(());
    let killed = watchdog.join().unwrap_or(false);
    let stdout = String::from_utf8_lossy(&out.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&err.join().unwrap_or_default()).into_owned();
    Ran { status, stdout, stderr, killed, wall_ms: started.elapsed().as_millis(), spawn_error: None, usage }
}

/// The checker answer's cost fields (`cost.mjs`): the process's CPU time, peak RSS and block writes, or null with the reason. A warm
/// check's process is the client that hands the program to its slot's daemon, whose use it does not include.
fn usage_fields(ran: &Ran, warm: bool) -> Value {
    match (warm, ran.usage) {
        (true, _) => json!({ "cpuMs": null, "maxRssKb": null, "writeKb": null, "usageWhy": "warm daemon" }),
        (false, Some(usage)) => json!({ "cpuMs": usage.cpu_ms, "maxRssKb": usage.max_rss_kb, "writeKb": usage.write_kb }),
        (false, None) => json!({ "cpuMs": null, "maxRssKb": null, "writeKb": null, "usageWhy": "no wait4 rusage on this platform" }),
    }
}

fn write_file(path: &str, text: &str) -> Res<()> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

struct TypecheckJob {
    dir: String,
    reply: Sender<Value>,
}

/// Gathers type checks into batches: a batch goes when it holds `max` projects or its first has waited `wait`; at most `processes`
/// batches run at once.
struct Batcher {
    queue: Mutex<Vec<TypecheckJob>>,
    ready: Condvar,
}
impl Batcher {
    fn submit(&self, dir: String) -> Receiver<Value> {
        let (reply, answer) = channel();
        self.queue.lock().unwrap().push(TypecheckJob { dir, reply });
        self.ready.notify_one();
        answer
    }
    fn start(self: &Arc<Self>, tsc: String, max: usize, wait: Duration, processes: Arc<Semaphore>) {
        let batcher = Arc::clone(self);
        thread::spawn(move || loop {
            let batch = {
                let mut queue = batcher.queue.lock().unwrap();
                while queue.is_empty() {
                    queue = batcher.ready.wait(queue).unwrap();
                }
                let first = Instant::now();
                while queue.len() < max && first.elapsed() < wait {
                    let (next, _) = batcher.ready.wait_timeout(queue, wait.saturating_sub(first.elapsed())).unwrap();
                    queue = next;
                }
                let take = queue.len().min(max);
                queue.drain(..take).collect::<Vec<_>>()
            };
            processes.acquire();
            let tsc = tsc.clone();
            let processes = Arc::clone(&processes);
            thread::spawn(move || {
                let dirs: Vec<String> = batch.iter().map(|job| job.dir.clone()).collect();
                match typecheck_batch(&tsc, &dirs) {
                    Some(answers) => {
                        for (job, answer) in batch.into_iter().zip(answers) {
                            let _ = job.reply.send(answer);
                        }
                    }
                    None => {
                        for job in batch {
                            let _ = job.reply.send(typecheck_one(&tsc, &job.dir));
                        }
                    }
                }
                processes.release();
            });
        });
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The checker (scripts/fuzz/checker.mjs: `runChecker`)

const CHECKER_ARGS: [&str; 9] = ["--format", "json", "--feedback-facts", "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"];
const ORDINARY_ARGS: [&str; 8] = ["--format", "json", "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"];

/// `init.warmChecker` (ADR 0543): a slot's feedback-facts check is answered by a retained checker daemon, one per slot directory
/// (the daemon's socket is keyed by the project path), instead of a fresh checker and producer per program. `Verify` also runs the
/// one-shot check at the same path after each warm one and reports whether stdout and the exit status are byte-identical.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Warm {
    Off,
    On,
    Verify,
}

struct Checker {
    bin: String,
    env: Vec<(String, String)>,
    warm: Warm,
    /// The slot directories a warm check ran in: their daemons are stopped when the executor ends.
    warmed: Mutex<std::collections::BTreeSet<String>>,
}

/// One worker for each of a check's own fan-outs (ADR 0594): the reactive IR's, the Type Facts producer's per-file passes and the
/// tsrs pool the producer parses on. The executor already runs a check per core, so a check that spreads over the cores only adds
/// thread starts and wake-ups (a third of a warm check's CPU). Answers do not depend on the worker counts. A value the executor's
/// caller set is kept; the executor's own type checks are not the checker's and keep theirs.
const CHECK_WORKERS: [(&str, &str); 3] = [("SOLID_CHECKER_IR_WORKERS", "1"), ("SOLID_TYPEFACTS_PER_FILE_WORKERS", "1"), ("RAYON_NUM_THREADS", "1")];

impl Checker {
    fn command(&self, dir: &str, args: &[&str], daemon: bool, program: Option<&str>) -> Command {
        let mut command = Command::new(&self.bin);
        command.arg("--project").arg(format!("{dir}/tsconfig.json")).args(args).current_dir(dir);
        for (key, value) in CHECK_WORKERS {
            if std::env::var_os(key).is_none() && !self.env.iter().any(|(given, _)| given == key) {
                command.env(key, value);
            }
        }
        for (key, value) in &self.env {
            command.env(key, value);
        }
        // The role log (ADR 0533) attributes the check's decisions to its program by this name, not by the directory: a warm slot's
        // directory is the same for every program it holds (`prepare_projects`).
        if let Some(program) = program {
            command.env("SOLID_CHECKER_IR_ROLE_LOG_PROGRAM", program);
        }
        command.env("SOLID_CHECKER_DAEMON", if daemon { "1" } else { "0" });
        // A slot's daemon lives while the executor does (it stops them at its end, `stop_warm`); the checker's two-minute default
        // would end one during a long Chrome wave and start it cold again.
        if daemon && !self.env.iter().any(|(key, _)| key == "SOLID_CHECKER_DAEMON_IDLE_SECS") {
            command.env("SOLID_CHECKER_DAEMON_IDLE_SECS", "900");
        }
        command
    }

    /// The checker on `dir` for `program`. `slotted`: `dir` is a slot the executor rewrites per program, which a warm daemon may serve.
    fn run(&self, dir: &str, ordinary: bool, slotted: bool, program: Option<&str>) -> Value {
        if !Path::new(&self.bin).exists() {
            return json!({ "spawnError": format!("checker binary not found: {}", self.bin), "dir": dir });
        }
        let warm = self.warm != Warm::Off && slotted && !ordinary;
        if warm {
            self.warmed.lock().unwrap().insert(dir.to_string());
        }
        let args = if ordinary { &ORDINARY_ARGS[..] } else { &CHECKER_ARGS[..] };
        let ran = run_command(self.command(dir, args, warm, program), Duration::from_secs(180));
        // A warm check that did not answer may leave its daemon working on this program: it is stopped (after it finishes what it
        // accepted) before the slot goes to the next program.
        if warm && (ran.killed || ran.spawn_error.is_some() || !matches!(ran.status, Some(0 | 1))) {
            self.stop(dir);
        }
        if let Some(error) = ran.spawn_error {
            return json!({ "spawnError": error, "dir": dir, "wallMs": ran.wall_ms as u64 });
        }
        let usage = usage_fields(&ran, warm);
        let mut answer = json!({ "status": ran.status, "stdout": ran.stdout, "stderr": ran.stderr, "wallMs": ran.wall_ms as u64, "dir": dir, "usage": usage });
        if warm && self.warm == Warm::Verify {
            let once = run_command(self.command(dir, args, false, program), Duration::from_secs(180));
            let identical = once.spawn_error.is_none() && !once.killed && once.status == ran.status && once.stdout == ran.stdout;
            answer["warm"] = json!({ "identical": identical, "oneShotStatus": once.status, "oneShotWallMs": once.wall_ms as u64 });
        } else if warm {
            answer["warm"] = json!({});
        }
        answer
    }

    /// Ends `dir`'s daemon once it has answered every check it accepted (`--stop-daemon`).
    fn stop(&self, dir: &str) {
        let mut args = vec!["--stop-daemon"];
        args.extend(CHECKER_ARGS);
        // The stop waits for what the daemon accepted, at most `SOLID_CHECKER_DAEMON_STOP_SECS` (60 s), then ends it by its pid.
        let ran = run_command(self.command(dir, &args, true, None), Duration::from_secs(120));
        if ran.killed || ran.status != Some(0) {
            eprintln!("fuzz-exec: could not stop the checker daemon of {dir}: {}", ran.stderr.trim());
        }
    }

    /// Stops every slot daemon this executor used.
    fn stop_warm(&self) {
        let dirs = std::mem::take(&mut *self.warmed.lock().unwrap());
        thread::scope(|scope| {
            for dir in &dirs {
                scope.spawn(move || self.stop(dir));
            }
        });
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The DevTools protocol over one WebSocket: an I/O thread reads and writes; a response goes to the channel its command named, an event
// to its session's channel.

struct Cdp {
    outgoing: Mutex<Sender<String>>,
    next: AtomicU64,
    pending: Mutex<HashMap<u64, Sender<Value>>>,
    sessions: Mutex<HashMap<String, Sender<Value>>>,
    /// The connection is gone (the socket closed, or the browser process exited, `fail`): every waiting command fails at once and
    /// no new one is sent. A dead browser once left a whole grid waiting with no error.
    closed: AtomicBool,
    socket: Mutex<Option<TcpStream>>,
}

impl Cdp {
    fn connect(url: &str) -> Res<Arc<Cdp>> {
        let address = url.trim_start_matches("ws://").split('/').next().ok_or("bad DevTools URL")?.to_string();
        let socket_address = address.parse().map_err(|e| format!("DevTools address {address}: {e}"))?;
        let stream = TcpStream::connect_timeout(&socket_address, Duration::from_secs(10)).map_err(|e| format!("connect {address}: {e}"))?;
        stream.set_nodelay(true).ok();
        let writer_stream = stream.try_clone().map_err(|e| e.to_string())?;
        let shutdown_stream = stream.try_clone().map_err(|e| e.to_string())?;
        // The handshake reads Chrome's answer: bounded, then the reader blocks for good (frames arrive when they do).
        stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
        let (mut socket, _) = tungstenite::client(url, stream).map_err(|e| format!("DevTools handshake: {e}"))?;
        socket.get_ref().set_read_timeout(None).ok();
        // One thread writes, one reads, each blocking: a second WebSocket over the same connection carries the outgoing frames (Chrome
        // never pings, so the reader never writes).
        let mut writer = tungstenite::WebSocket::from_raw_socket(writer_stream, tungstenite::protocol::Role::Client, None);
        let (outgoing, queue) = channel::<String>();
        thread::spawn(move || {
            for text in queue {
                if writer.send(tungstenite::Message::Text(text.into())).is_err() {
                    return;
                }
            }
        });
        let cdp = Arc::new(Cdp {
            outgoing: Mutex::new(outgoing),
            next: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            closed: AtomicBool::new(false),
            socket: Mutex::new(Some(shutdown_stream)),
        });
        let router = Arc::clone(&cdp);
        thread::spawn(move || {
            loop {
                match socket.read() {
                    Ok(tungstenite::Message::Text(text)) => {
                        let Ok(message) = serde_json::from_str::<Value>(&text) else { continue };
                        if let Some(id) = message.get("id").and_then(Value::as_u64) {
                            if let Some(reply) = router.pending.lock().unwrap().remove(&id) {
                                let _ = reply.send(message);
                            }
                        } else if let Some(session) = message.get("sessionId").and_then(Value::as_str) {
                            if let Some(events) = router.sessions.lock().unwrap().get(session) {
                                let _ = events.send(message);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(_) => {
                        // The browser is gone: every waiting command fails.
                        router.fail();
                        return;
                    }
                }
            }
        });
        Ok(cdp)
    }

    /// The connection is over: no command is sent again, every waiting one fails now (its reply channel dropped), and the socket is
    /// shut so the reader and the writer stop. Idempotent.
    fn fail(&self) {
        self.closed.store(true, Ordering::SeqCst);
        if let Some(socket) = self.socket.lock().unwrap().take() {
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
        self.pending.lock().unwrap().clear();
        self.sessions.lock().unwrap().clear();
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Sends a command; its response goes to `reply`. Returns the command's id. On a closed connection nothing is sent and `reply`
    /// is dropped, so its receiver fails at once.
    fn send(&self, session: Option<&str>, method: &str, params: Value, reply: Sender<Value>) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        if self.is_closed() {
            return id;
        }
        self.pending.lock().unwrap().insert(id, reply);
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = json!(session);
        }
        let _ = self.outgoing.lock().unwrap().send(message.to_string());
        id
    }

    fn call(&self, session: Option<&str>, method: &str, params: Value) -> Res<Value> {
        if self.is_closed() {
            return Err(format!("{method}: the browser is gone"));
        }
        let (reply, answer) = channel();
        let id = self.send(session, method, params, reply);
        match answer.recv_timeout(Duration::from_secs(30)) {
            Ok(message) => match message.get("error") {
                Some(error) => Err(format!("{method}: {}", error.get("message").and_then(Value::as_str).unwrap_or("error"))),
                None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
            },
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!("{method}: no response"))
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Chrome: the browser, its pages, the Vite helper

/// Playwright 1.60's switches for chromium headless shell (`chromium.launch({ headless: true, args: ["--in-process-gpu"] })`), as
/// chrome-host.mjs launches it; the DevTools endpoint is a port here instead of Playwright's pipe.
const BROWSER_ARGS: &[&str] = &[
    "--disable-field-trial-config", "--disable-background-networking", "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows", "--disable-back-forward-cache", "--disable-breakpad",
    "--disable-client-side-phishing-detection", "--disable-component-extensions-with-background-pages", "--disable-component-update",
    "--no-default-browser-check", "--disable-default-apps", "--disable-dev-shm-usage", "--disable-edgeupdater", "--disable-extensions",
    "--disable-features=AvoidUnnecessaryBeforeUnloadCheckSync,BoundaryEventDispatchTracksNodeRemoval,DestroyProfileOnBrowserClose,DialMediaRouteProvider,GlobalMediaControls,HttpsUpgrades,LensOverlay,MediaRouter,PaintHolding,ThirdPartyStoragePartitioning,Translate,AutoDeElevate,RenderDocument,OptimizationHints,msForceBrowserSignIn,msEdgeUpdateLaunchServicesPreferredVersion",
    "--enable-features=CDPScreenshotNewSurface", "--allow-pre-commit-input", "--disable-hang-monitor", "--disable-ipc-flooding-protection",
    "--disable-popup-blocking", "--disable-prompt-on-repost", "--disable-renderer-backgrounding", "--force-color-profile=srgb",
    "--metrics-recording-only", "--no-first-run", "--password-store=basic", "--use-mock-keychain", "--no-service-autorun",
    "--export-tagged-pdf", "--disable-search-engine-choice-screen", "--unsafely-disable-devtools-self-xss-warnings",
    "--edge-skip-compat-layer-relaunch", "--disable-infobars", "--disable-search-engine-choice-screen", "--disable-sync",
    "--enable-unsafe-swiftshader", "--headless", "--hide-scrollbars", "--mute-audio",
    "--blink-settings=primaryHoverType=2,availableHoverTypes=2,primaryPointerType=4,availablePointerTypes=4", "--no-sandbox",
    "--in-process-gpu",
];

const VIRTUAL_BUDGET_MS: u64 = 5000;
const VIRTUAL_CAP_MS: u64 = 60000;
const PAGE_TIMEOUT: Duration = Duration::from_secs(30);
/// What a page that spent VIRTUAL_CAP_MS still gets of wall time for a record already in flight (`run_on`).
const CAP_GRACE: Duration = Duration::from_secs(1);

struct Chrome {
    cdp: Arc<Cdp>,
    base: String,
    control: String,
    work: PathBuf,
    runs: AtomicU64,
    pages: usize,
    idle: Mutex<Vec<Page>>,
    font_families: Value,
    browser: Arc<Mutex<std::process::Child>>,
    vite: Arc<Mutex<std::process::Child>>,
    profile: PathBuf,
    /// A child (the browser or the Vite helper) exited: the supervisor logged it and failed the connection (`supervise`).
    dead: Arc<AtomicBool>,
    stopping: Arc<AtomicBool>,
}

struct Page {
    context: String,
    session: String,
    frame: String,
    events: Receiver<Value>,
    reply: Sender<Value>,
}

/// The longest a call to the Vite helper may take: a staging or a source-map lookup is milliseconds; a helper that hangs (a full disk,
/// a stalled event loop) must fail the call, not the grid.
const HELPER_TIMEOUT: Duration = Duration::from_secs(120);

fn http_post(base: &str, path: &str, body: &str) -> Res<String> {
    let address = base.trim_start_matches("http://").trim_end_matches('/').to_string();
    let socket_address = std::net::ToSocketAddrs::to_socket_addrs(&address).map_err(|e| format!("vite helper {address}: {e}"))?.next().ok_or(format!("vite helper {address}: no address"))?;
    let mut stream = TcpStream::connect_timeout(&socket_address, Duration::from_secs(10)).map_err(|e| format!("vite helper {address}: {e}"))?;
    stream.set_read_timeout(Some(HELPER_TIMEOUT)).ok();
    stream.set_write_timeout(Some(HELPER_TIMEOUT)).ok();
    write!(stream, "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|e| format!("vite helper {path}: {e}"))?;
    let text = String::from_utf8_lossy(&response).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").ok_or("vite helper: malformed response")?;
    if !head.starts_with("HTTP/1.1 200") {
        return Err(format!("vite helper {path}: {}", head.lines().next().unwrap_or("")));
    }
    Ok(body.to_string())
}

/// The first line `reader` prints, within `limit`; EOF or the deadline is an error (the reading thread is left to end with the pipe).
fn read_line_within(reader: impl Read + Send + 'static, limit: Duration) -> Res<String> {
    let (line, read) = channel();
    thread::spawn(move || {
        let mut text = String::new();
        let outcome = BufReader::new(reader).read_line(&mut text).map(|n| (n, text));
        let _ = line.send(outcome);
    });
    match read.recv_timeout(limit) {
        Ok(Ok((0, _))) => Err("exited before it was ready".into()),
        Ok(Ok((_, text))) => Ok(text),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err(format!("not ready within {} s", limit.as_secs())),
    }
}

/// Watches a child of the executor: when it exits on its own, logs its status (and the browser's stderr tail), reaps it, marks the
/// executor's browser dead and fails the DevTools connection, so every page waiting on it fails now and the batch can restart it
/// (`Executor::run`). Before, an exited browser stayed a zombie and a grid waited on it with no error.
fn supervise(name: &'static str, child: Arc<Mutex<std::process::Child>>, tail: Option<Arc<Mutex<std::collections::VecDeque<String>>>>, cdp: Arc<Cdp>, dead: Arc<AtomicBool>, stopping: Arc<AtomicBool>) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(250));
            if stopping.load(Ordering::SeqCst) {
                return;
            }
            let status = child.lock().unwrap().try_wait();
            match status {
                Ok(None) => {}
                Ok(Some(status)) => {
                    let tail = tail.map(|tail| tail.lock().unwrap().iter().cloned().collect::<Vec<_>>().join(" | ")).unwrap_or_default();
                    eprintln!("fuzz-exec: {name} exited ({status}){}", if tail.is_empty() { String::new() } else { format!("; its last stderr lines: {tail}") });
                    dead.store(true, Ordering::SeqCst);
                    cdp.fail();
                    return;
                }
                Err(error) => {
                    eprintln!("fuzz-exec: {name}: cannot wait on it: {error}");
                    return;
                }
            }
        }
    });
}

impl Chrome {
    /// Kills both children and waits on them (no zombie), and removes the profile and the work directory.
    fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.cdp.fail();
        for child in [&self.browser, &self.vite] {
            let mut child = child.lock().unwrap();
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.profile);
        let _ = std::fs::remove_dir_all(&self.work);
    }

    fn is_dead(&self) -> bool {
        self.dead.load(Ordering::SeqCst) || self.cdp.is_closed()
    }

    fn start(settings: &Value) -> Res<Chrome> {
        let text = |key: &str| settings.get(key).and_then(Value::as_str).map(str::to_string).ok_or(format!("chrome.{key} missing"));
        // The Vite helper: one line `{ base, work }` once it serves.
        let mut vite = spawn(Command::new(text("node")?).arg(text("viteHost")?).arg(settings.to_string()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::inherit()))
            .map_err(|e| format!("vite helper: {e}"))?;
        let helper_out = vite.stdout.take().unwrap();
        let line = read_line_within(helper_out, Duration::from_secs(120)).map_err(|e| {
            let _ = vite.kill();
            let _ = vite.wait();
            format!("vite helper: {e}")
        })?;
        let ready: Value = serde_json::from_str(line.trim()).map_err(|e| format!("vite helper said {line:?}: {e}"))?;
        let base = ready["base"].as_str().ok_or("vite helper: no base")?.to_string();
        let work = PathBuf::from(ready["work"].as_str().ok_or("vite helper: no work")?);
        let control = ready["control"].as_str().ok_or("vite helper: no control")?.to_string();

        static STARTS: AtomicU64 = AtomicU64::new(0);
        let profile = std::env::temp_dir().join(format!("fuzz-exec-profile-{}-{}", std::process::id(), STARTS.fetch_add(1, Ordering::Relaxed)));
        let mut browser = spawn(
            Command::new(text("browser")?)
                .args(BROWSER_ARGS)
                .arg(format!("--user-data-dir={}", profile.display()))
                .arg("--remote-debugging-port=0")
                .arg("--no-startup-window")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped()),
        )
        .map_err(|e| format!("browser: {e}"))?;
        // The browser's stderr: its DevTools endpoint, then a bounded tail kept for the log of an exit (`supervise`).
        let tail = Arc::new(Mutex::new(std::collections::VecDeque::<String>::new()));
        let (endpoint, found) = channel::<Option<String>>();
        {
            let tail = Arc::clone(&tail);
            let mut stderr = BufReader::new(browser.stderr.take().unwrap());
            thread::spawn(move || {
                let mut endpoint = Some(endpoint);
                let mut line = String::new();
                while stderr.read_line(&mut line).is_ok_and(|n| n > 0) {
                    if let Some(at) = line.find("ws://") {
                        if let Some(endpoint) = endpoint.take() {
                            let _ = endpoint.send(Some(line[at..].trim().to_string()));
                        }
                    }
                    let mut tail = tail.lock().unwrap();
                    tail.push_back(line.trim_end().chars().take(400).collect());
                    if tail.len() > 20 {
                        tail.pop_front();
                    }
                    line.clear();
                }
                if let Some(endpoint) = endpoint.take() {
                    let _ = endpoint.send(None);
                }
            });
        }
        let url = match found.recv_timeout(Duration::from_secs(60)) {
            Ok(Some(url)) => url,
            outcome => {
                let _ = browser.kill();
                let _ = browser.wait();
                let _ = vite.kill();
                let _ = vite.wait();
                return Err(if matches!(outcome, Ok(None)) { "the browser exited before it listened".into() } else { "the browser printed no DevTools endpoint within 60 s".into() });
            }
        };
        let cdp = Cdp::connect(&url)?;
        let dead = Arc::new(AtomicBool::new(false));
        let stopping = Arc::new(AtomicBool::new(false));
        let browser = Arc::new(Mutex::new(browser));
        let vite = Arc::new(Mutex::new(vite));
        supervise("the browser", Arc::clone(&browser), Some(tail), Arc::clone(&cdp), Arc::clone(&dead), Arc::clone(&stopping));
        supervise("the Vite helper", Arc::clone(&vite), None, Arc::clone(&cdp), Arc::clone(&dead), Arc::clone(&stopping));
        let chrome = Chrome {
            cdp,
            base,
            control,
            work,
            runs: AtomicU64::new(0),
            pages: settings.get("pages").and_then(Value::as_u64).unwrap_or(16) as usize,
            idle: Mutex::new(Vec::new()),
            font_families: settings.get("fontFamilies").cloned().unwrap_or(Value::Null),
            browser,
            vite,
            profile,
            dead,
            stopping,
        };
        // Warm the dependency bundles and the runner once, then freeze the runner as transformed (chrome-host.mjs `start`). A failure
        // here stops both children (no browser left behind a failed start).
        let page = match chrome.open_page() {
            Ok(page) => page,
            Err(error) => {
                chrome.stop();
                return Err(error);
            }
        };
        let warm = (|| -> Res<()> {
            let (reply, answer) = channel();
            chrome.cdp.send(Some(&page.session), "Page.navigate", json!({ "url": format!("{}index.html?id=__warm", chrome.base), "frameId": page.frame, "referrerPolicy": "unsafeUrl" }), reply);
            answer.recv_timeout(PAGE_TIMEOUT).map_err(|_| "warm navigation: no response")?;
            let deadline = Instant::now() + PAGE_TIMEOUT;
            loop {
                let event = page.events.recv_timeout(deadline.saturating_duration_since(Instant::now())).map_err(|_| "warm page: no load event")?;
                if event.get("method").and_then(Value::as_str) == Some("Page.loadEventFired") {
                    return Ok(());
                }
            }
        })();
        chrome.close_page(page);
        if let Err(error) = warm.and_then(|()| http_post(&chrome.control, "/__sc/freeze", "{}").map(|_| ())) {
            chrome.stop();
            return Err(error);
        }
        Ok(chrome)
    }

    /// A browser context and page with Playwright's page setup (newContext + newPage).
    fn open_page(&self) -> Res<Page> {
        let cdp = &self.cdp;
        let context = cdp.call(None, "Target.createBrowserContext", json!({ "disposeOnDetach": true }))?["browserContextId"].as_str().ok_or("no context id")?.to_string();
        let target = cdp.call(None, "Target.createTarget", json!({ "url": "about:blank", "browserContextId": context }))?["targetId"].as_str().ok_or("no target id")?.to_string();
        let session = cdp.call(None, "Target.attachToTarget", json!({ "targetId": target, "flatten": true }))?["sessionId"].as_str().ok_or("no session id")?.to_string();
        let (reply, events) = channel();
        cdp.sessions.lock().unwrap().insert(session.clone(), reply.clone());
        let s = Some(session.as_str());
        cdp.call(s, "Page.enable", json!({}))?;
        let frame = cdp.call(s, "Page.getFrameTree", json!({}))?["frameTree"]["frame"]["id"].as_str().ok_or("no frame id")?.to_string();
        cdp.call(s, "Page.setLifecycleEventsEnabled", json!({ "enabled": true }))?;
        // Playwright also enables Runtime and Network here, to report console messages and requests. They only observe (Chrome then
        // serializes an event per request and console call, its stack captured), the runner reads neither, and the records of 1,536
        // programs were identical without them; the page is 7% faster.
        cdp.call(s, "Emulation.setFocusEmulationEnabled", json!({ "enabled": true }))?;
        let window = cdp.call(None, "Browser.getWindowForTarget", json!({ "targetId": target }))?["windowId"].clone();
        cdp.call(None, "Browser.setWindowBounds", json!({ "windowId": window, "bounds": { "width": 1280, "height": 720 } }))?;
        cdp.call(s, "Emulation.setDeviceMetricsOverride", json!({ "mobile": false, "width": 1280, "height": 720, "screenWidth": 1280, "screenHeight": 720, "deviceScaleFactor": 1, "screenOrientation": { "angle": 0, "type": "landscapePrimary" } }))?;
        if !self.font_families.is_null() {
            cdp.call(s, "Page.setFontFamilies", self.font_families.clone())?;
        }
        cdp.call(s, "Emulation.setEmulatedMedia", json!({ "media": "", "features": [
            { "name": "prefers-color-scheme", "value": "light" }, { "name": "prefers-reduced-motion", "value": "no-preference" },
            { "name": "forced-colors", "value": "none" }, { "name": "prefers-contrast", "value": "no-preference" } ] }))?;
        Ok(Page { context, session, frame, events, reply })
    }

    fn close_page(&self, page: Page) {
        self.cdp.sessions.lock().unwrap().remove(&page.session);
        let _ = self.cdp.call(None, "Target.disposeBrowserContext", json!({ "browserContextId": page.context }));
    }

    /// One program on `page`: a fresh budget, a navigation to a fresh document, the record the page hands back, virtual time paused.
    fn run_on(&self, page: &Page, id: &str) -> Res<Value> {
        let cdp = &self.cdp;
        let s = Some(page.session.as_str());
        while page.events.try_recv().is_ok() {}
        let mut granted = VIRTUAL_BUDGET_MS;
        let policy = |budget: u64| json!({ "policy": "pauseIfNetworkFetchesPending", "budget": budget });
        cdp.call(s, "Emulation.setVirtualTimePolicy", policy(VIRTUAL_BUDGET_MS))?;
        let run = self.runs.fetch_add(1, Ordering::Relaxed) + 1;
        let url = format!("{}sc.html?run={run}#id={id}&clock=real", self.base);
        let navigated = cdp.call(s, "Page.navigate", json!({ "url": url, "frameId": page.frame, "referrerPolicy": "unsafeUrl" }))?;
        if let Some(error) = navigated.get("errorText").and_then(Value::as_str) {
            return Err(format!("navigation: {error}"));
        }
        let loader = navigated.get("loaderId").and_then(Value::as_str).unwrap_or("").to_string();
        let mut deadline = Instant::now() + PAGE_TIMEOUT;
        let mut evaluate: Option<u64> = None;
        // Until the new document commits, the last program's document still runs and may spend budgets; those grants do not count
        // toward this program's cap (the old document is gone at the commit).
        let mut committed = false;
        let mut loaded = false;
        let grant = |granted: &mut u64, committed: bool| {
            if !committed || *granted < VIRTUAL_CAP_MS {
                if committed {
                    *granted += VIRTUAL_BUDGET_MS;
                }
                cdp.send(s, "Emulation.setVirtualTimePolicy", policy(VIRTUAL_BUDGET_MS), page.reply.clone());
            }
        };
        let record = loop {
            // In slices of a second: a browser that died is seen at once, not at the deadline (the page's own reply sender keeps
            // its event channel open).
            let left = deadline.saturating_duration_since(Instant::now());
            let message = match page.events.recv_timeout(left.min(Duration::from_secs(1))) {
                Ok(message) => message,
                Err(RecvTimeoutError::Timeout) if cdp.is_closed() => return Err("the browser is gone".into()),
                Err(RecvTimeoutError::Timeout) if !left.is_zero() => continue,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!("no record (committed {committed}, loaded {loaded}, record asked {}, virtual time granted {granted} ms)", evaluate.is_some()));
                }
                Err(RecvTimeoutError::Disconnected) => return Err("the browser closed".into()),
            };
            match message.get("method").and_then(Value::as_str) {
                Some("Emulation.virtualTimeBudgetExpired") => {
                    // A page that spent the whole cap and asks for more is paused for good: its record can only be one already in
                    // flight. It gets CAP_GRACE of wall time for that, not the rest of PAGE_TIMEOUT, then fails and is run again on a
                    // fresh page, where the same program has always finished (2026-10-30: about one such page per two-code grid,
                    // each holding its batch idle for 30 s).
                    if committed && granted >= VIRTUAL_CAP_MS {
                        deadline = deadline.min(Instant::now() + CAP_GRACE);
                    }
                    grant(&mut granted, committed)
                }
                Some("Page.lifecycleEvent") if !committed && message["params"]["loaderId"] == loader.as_str() => {
                    committed = true;
                    if message["params"]["name"] == "DOMContentLoaded" {
                        loaded = true;
                        evaluate = Some(cdp.send(s, "Runtime.evaluate", json!({ "expression": "globalThis.__scRecordPromise", "awaitPromise": true, "returnByValue": true, "userGesture": true }), page.reply.clone()));
                    }
                }
                // `goto(..., { waitUntil: "domcontentloaded" })`, then `evaluate(() => globalThis.__scRecordPromise)`.
                Some("Page.lifecycleEvent")
                    if evaluate.is_none()
                        && message["params"]["name"] == "DOMContentLoaded"
                        && message["params"]["frameId"] == page.frame.as_str()
                        && message["params"]["loaderId"] == loader.as_str() =>
                {
                    loaded = true;
                    evaluate = Some(cdp.send(s, "Runtime.evaluate", json!({ "expression": "globalThis.__scRecordPromise", "awaitPromise": true, "returnByValue": true, "userGesture": true }), page.reply.clone()));
                }
                None if evaluate.is_some() && message.get("id").and_then(Value::as_u64) == evaluate => {
                    if let Some(error) = message.get("error") {
                        return Err(format!("evaluate: {error}"));
                    }
                    if let Some(details) = message["result"].get("exceptionDetails") {
                        return Err(format!("evaluate threw: {}", details["exception"]["description"].as_str().unwrap_or("?")));
                    }
                    break message["result"]["result"]["value"].clone();
                }
                _ => {}
            }
        };
        cdp.call(s, "Emulation.setVirtualTimePolicy", json!({ "policy": "pause" }))?;
        let mapped = http_post(&self.control, &format!("/__sc/map?id={id}"), &record.to_string())?;
        serde_json::from_str(&mapped).map_err(|e| format!("mapped record: {e}"))
    }

    fn run(self: &Arc<Self>, cases: Vec<Value>) -> Value {
        let failed = |id: &str, message: String| json!({ "id": id, "failure": { "phase": "chrome-host", "message": message.chars().take(800).collect::<String>() }, "diagnostics": [], "errors": [], "witnesses": [] });
        let mut records = serde_json::Map::new();
        let mut staged = Vec::new();
        let mut stage = Vec::new();
        for case in cases {
            let id = case["id"].as_str().unwrap_or_default().to_string();
            if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
                records.insert(id.clone(), failed(&id, format!("unsafe case id {id}")));
                continue;
            }
            staged.push(id);
            stage.push(case);
        }
        // The helper stages them where the pages load them (chrome-host.mjs `stage`: in memory, a case with a package on disk).
        if let Err(error) = http_post(&self.control, "/__sc/stage", &Value::Array(stage).to_string()) {
            for id in staged.drain(..) {
                records.insert(id.clone(), failed(&id, format!("staging: {error}")));
            }
        }
        let queue = Arc::new(Mutex::new(staged.clone().into_iter()));
        let results = Arc::new(Mutex::new(Vec::new()));
        let workers: Vec<_> = (0..self.pages.min(staged.len()))
            .map(|_| {
                let chrome = Arc::clone(self);
                let queue = Arc::clone(&queue);
                let results = Arc::clone(&results);
                thread::spawn(move || {
                    let mut page = chrome.idle.lock().unwrap().pop();
                    loop {
                        let Some(id) = queue.lock().unwrap().next() else { break };
                        if page.is_none() {
                            match chrome.open_page() {
                                Ok(opened) => page = Some(opened),
                                Err(error) => {
                                    results.lock().unwrap().push((id.clone(), failed(&id, error)));
                                    continue;
                                }
                            }
                        }
                        // A page that gave no record is the host's failure, not the program's: once more, on a fresh page.
                        let mut outcome = chrome.run_on(page.as_ref().unwrap(), &id);
                        if let Err(error) = &outcome {
                            // The first attempt's error is the only trace of a page that hung (each costs PAGE_TIMEOUT of the batch).
                            eprintln!("fuzz-exec: {id}: retried on a fresh page: {error}");
                        }
                        if outcome.is_err() {
                            chrome.close_page(page.take().unwrap());
                            outcome = chrome.open_page().and_then(|fresh| {
                                let again = chrome.run_on(&fresh, &id);
                                page = Some(fresh);
                                again
                            });
                        }
                        match outcome {
                            Ok(record) => results.lock().unwrap().push((id, record)),
                            Err(error) => {
                                results.lock().unwrap().push((id.clone(), failed(&id, error)));
                                if let Some(page) = page.take() {
                                    chrome.close_page(page);
                                }
                            }
                        }
                    }
                    if let Some(page) = page {
                        chrome.idle.lock().unwrap().push(page);
                    }
                })
            })
            .collect();
        for worker in workers {
            let _ = worker.join();
        }
        for (id, record) in results.lock().unwrap().drain(..) {
            records.insert(id, record);
        }
        let _ = http_post(&self.control, "/__sc/done", &json!(staged).to_string());
        Value::Object(records)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The protocol loop

/// The fixed project directories a program's step 1 borrows (a clean and a twin project): rewritten in place per program instead of
/// created and removed, so a grid's file churn is a few rewritten files per program.
struct Slots {
    free: Mutex<Vec<(SlotDir, SlotDir)>>,
    ready: Condvar,
}
impl Slots {
    /// A free pair, preferring one whose clean slot last held a project of the same `shape` (`project_shape`): a warm checker's
    /// daemon updates the files of such a project in place, and reopens its whole program for any other (ADR 0592).
    fn take(&self, shape: Option<&str>) -> (SlotDir, SlotDir) {
        let mut free = self.free.lock().unwrap();
        loop {
            let same = shape.and_then(|shape| free.iter().rposition(|(slot, _)| slot.shape.as_deref() == Some(shape)));
            if let Some(pair) = same.map(|at| free.remove(at)).or_else(|| free.pop()) {
                return pair;
            }
            free = self.ready.wait(free).unwrap();
        }
    }
    fn give(&self, pair: (SlotDir, SlotDir)) {
        self.free.lock().unwrap().push(pair);
        self.ready.notify_one();
    }
    /// The shapes the free slots hold, the last freed first.
    fn free_shapes(&self) -> Vec<String> {
        self.free.lock().unwrap().iter().rev().filter_map(|(slot, _)| slot.shape.clone()).collect()
    }
}

/// One slot directory, `<slotRoot>/<n>/<program>`: the project lives in a directory named for the program it holds, renamed (its files
/// kept, so `sync_files` still rewrites only what changed) when the slot takes the next program. The checker's paths then name the
/// program: the role log (roles.mjs `programOf`) keys each decision by the directory above the file, and with one fixed directory
/// per slot every program of a grid folded as one ("slots"), so decisions of different programs deduplicated against each other.
struct SlotDir {
    parent: PathBuf,
    name: Option<String>,
    /// The `project_shape` of the project the slot holds.
    shape: Option<String>,
}
impl SlotDir {
    fn new(parent: PathBuf) -> Self {
        Self { parent, name: None, shape: None }
    }
    /// The slot's directory for program `id`, the previous program's directory renamed to it.
    fn place(&mut self, id: &str) -> std::io::Result<PathBuf> {
        let name = slot_name(id);
        let target = self.parent.join(&name);
        match self.name.as_deref() {
            Some(current) if current == name => {}
            Some(current) if self.parent.join(current).is_dir() => std::fs::rename(self.parent.join(current), &target)?,
            _ => std::fs::create_dir_all(&target)?,
        }
        self.name = Some(name);
        Ok(target)
    }
}

/// What decides whether a retained checker daemon reopens its program when a slot takes a project (ADR 0592): its file names and
/// its tsconfig.json. The daemon rebuilds on a changed tsconfig or a changed directory listing, and updates the files of the program
/// it holds otherwise. Only the order in which slots are handed out reads it, never an answer.
fn project_shape(files: &serde_json::Map<String, Value>) -> String {
    let mut names = files.keys().map(String::as_str).collect::<Vec<_>>();
    names.sort_unstable();
    let tsconfig = files.get("tsconfig.json").and_then(Value::as_str).unwrap_or_default();
    format!("{}\0{tsconfig}", names.join("\0"))
}

/// The `project_shape` of a request's project when it borrows a slot (not one with a directory of its own).
fn slot_shape(request: &Value) -> Option<String> {
    let project = request["project"].as_object().filter(|project| project.get("dir").is_none())?;
    project.get("files").and_then(Value::as_object).map(project_shape)
}

/// Step-1 requests waiting for a worker (ADR 0592). A worker takes the oldest of the first `JOB_WINDOW` whose project has the shape a
/// free slot last held, so the slot's warm daemon updates its program instead of reopening it; else the oldest. Each request is passed
/// over at most `limit` times, so none waits for ever behind a run of others. The order in which programs are prepared changes no
/// answer: each is its own request, answered by its own `seq`.
struct Jobs {
    state: Mutex<(std::collections::VecDeque<(Value, Option<String>, usize)>, bool)>,
    ready: Condvar,
    limit: usize,
}
const JOB_WINDOW: usize = 64;
impl Jobs {
    fn new(limit: usize) -> Self {
        Self { state: Mutex::new((Default::default(), false)), ready: Condvar::new(), limit }
    }
    fn push(&self, request: Value) {
        let shape = slot_shape(&request);
        self.state.lock().unwrap().0.push_back((request, shape, 0));
        self.ready.notify_one();
    }
    /// No more requests: the workers end once the queue is empty.
    fn close(&self) {
        self.state.lock().unwrap().1 = true;
        self.ready.notify_all();
    }
    /// The next request for a worker, `free` naming the shapes of the free slots; `None` once closed and empty.
    fn next(&self, free: impl Fn() -> Vec<String>) -> Option<Value> {
        let mut state = self.state.lock().unwrap();
        loop {
            if !state.0.is_empty() {
                let free = free();
                let queue = &mut state.0;
                let window = queue.len().min(JOB_WINDOW);
                let wanted = (0..window).find(|&at| queue[at].1.as_ref().is_some_and(|shape| free.contains(shape))).unwrap_or(0);
                let at = (0..wanted).find(|&at| queue[at].2 >= self.limit).unwrap_or(wanted);
                for passed in queue.iter_mut().take(at) {
                    passed.2 += 1;
                }
                return queue.remove(at).map(|(request, _, _)| request);
            }
            if state.1 {
                return None;
            }
            state = self.ready.wait(state).unwrap();
        }
    }
}

/// A program id as one directory name: `[A-Za-z0-9_-]` and inner dots as they are, every other byte `%XX` (roles.mjs decodes it with
/// `decodeURIComponent`). An id that would pass 200 bytes keeps its first 160 and an FNV-1a hash of the whole: still one name per id.
fn slot_name(id: &str) -> String {
    let mut name = String::new();
    for (index, byte) in id.bytes().enumerate() {
        if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-' || (byte == b'.' && index > 0) {
            name.push(byte as char);
        } else {
            name.push_str(&format!("%{byte:02X}"));
        }
    }
    if name.is_empty() {
        return "%00".into();
    }
    if name.len() > 200 {
        let hash = id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3));
        let mut cut = 160;
        while !name.is_char_boundary(cut) || name[..cut].ends_with('%') || name[..cut.saturating_sub(1)].ends_with('%') {
            cut -= 1;
        }
        name = format!("{}~{hash:016x}", &name[..cut]);
    }
    name
}

/// Makes `dir` hold exactly `files` (top-level names): other entries removed, a changed file rewritten in place, an equal one untouched.
fn sync_files(dir: &Path, files: &serde_json::Map<String, Value>) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !files.contains_key(&*entry.file_name().to_string_lossy()) {
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
    }
    for (name, text) in files {
        let text = text.as_str().unwrap_or_default();
        let target = dir.join(name);
        if std::fs::read(&target).is_ok_and(|current| current == text.as_bytes()) {
            continue;
        }
        std::fs::write(&target, text)?;
    }
    Ok(())
}

/// Writes `files` (paths relative to `dir`, nested ones too) into a fresh `dir`.
fn write_project(dir: &Path, files: &serde_json::Map<String, Value>) -> Res<()> {
    let _ = std::fs::remove_dir_all(dir);
    for (name, text) in files {
        write_file(&dir.join(name).to_string_lossy(), text.as_str().unwrap_or_default())?;
    }
    Ok(())
}

struct Executor {
    out: Arc<Out>,
    slots: Slots,
    batcher: Arc<Batcher>,
    checker: Arc<Checker>,
    checks: Arc<Semaphore>,
    keep_work: bool,
    /// The browser in use, replaced when it dies (`restart_chrome`), with the settings it started from and the restarts so far.
    chrome: Option<Mutex<Arc<Chrome>>>,
    chrome_settings: Value,
    restarts: AtomicU64,
}

/// How many times one executor starts a new browser after one died: each is logged; past the limit a dead browser's programs are the
/// host's failures (harness rows, never verdicts), so a machine that kills browsers stops the run instead of looping.
const MAX_RESTARTS: u64 = 3;

impl Executor {
    fn init(out: Arc<Out>, request: &Value) -> Res<Executor> {
        let text = |key: &str| request.get(key).and_then(Value::as_str).map(str::to_string).ok_or(format!("init.{key} missing"));
        let concurrency = request.get("concurrency").and_then(Value::as_u64).unwrap_or(8) as usize;
        let tsc = text("tsc")?;
        let batcher = Arc::new(Batcher { queue: Mutex::new(Vec::new()), ready: Condvar::new() });
        let batch = request.get("typecheckBatch").and_then(Value::as_u64).unwrap_or(24) as usize;
        batcher.start(tsc.clone(), batch, Duration::from_millis(15), Arc::new(Semaphore::new(concurrency)));
        let env = request.get("env").and_then(Value::as_object).map(|env| env.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string()))).collect()).unwrap_or_default();
        let warm = match request.get("warmChecker").and_then(Value::as_str) {
            None | Some("off") => Warm::Off,
            Some("on") => Warm::On,
            Some("verify") => Warm::Verify,
            Some(other) => return Err(format!("init.warmChecker is off, on or verify, not {other}")),
        };
        let checker = Arc::new(Checker { bin: text("checkerBin")?, env, warm, warmed: Mutex::new(Default::default()) });
        let slot_root = PathBuf::from(text("slotRoot")?);
        let slots = Slots {
            free: Mutex::new((0..concurrency * 2).map(|slot| (SlotDir::new(slot_root.join(format!("{slot}"))), SlotDir::new(slot_root.join(format!("{slot}-run"))))).collect()),
            ready: Condvar::new(),
        };
        let chrome_settings = request.get("chrome").cloned().unwrap_or(Value::Null);
        let chrome = if chrome_settings.is_null() { None } else { Some(Mutex::new(Arc::new(Chrome::start(&chrome_settings)?))) };
        Ok(Executor {
            out,
            slots,
            batcher,
            checker,
            checks: Arc::new(Semaphore::new(concurrency)),
            keep_work: request.get("keepWork").and_then(Value::as_bool).unwrap_or(false),
            chrome,
            chrome_settings,
            restarts: AtomicU64::new(0),
        })
    }

    fn current_chrome(&self) -> Option<Arc<Chrome>> {
        self.chrome.as_ref().map(|slot| Arc::clone(&slot.lock().unwrap()))
    }

    /// A new browser in place of `dead`, once per death (a run that finds it already replaced takes the new one), at most
    /// MAX_RESTARTS times. `None` when the limit is spent or the new browser does not start.
    fn restart_chrome(&self, dead: &Arc<Chrome>) -> Option<Arc<Chrome>> {
        let slot = self.chrome.as_ref()?;
        let mut current = slot.lock().unwrap();
        if !Arc::ptr_eq(&current, dead) {
            return Some(Arc::clone(&current));
        }
        let restart = self.restarts.fetch_add(1, Ordering::SeqCst) + 1;
        if restart > MAX_RESTARTS {
            eprintln!("fuzz-exec: the browser died again; {MAX_RESTARTS} restarts spent, its programs are harness failures");
            return None;
        }
        dead.stop();
        match Chrome::start(&self.chrome_settings) {
            Ok(fresh) => {
                eprintln!("fuzz-exec: browser restart {restart} of {MAX_RESTARTS}");
                *current = Arc::new(fresh);
                Some(Arc::clone(&current))
            }
            Err(error) => {
                eprintln!("fuzz-exec: browser restart {restart} failed: {error}");
                None
            }
        }
    }

    /// Runs `cases` on the browser; when it died during the batch, the programs it failed run once more on a new browser (the
    /// records that came back stand: a program's record never mixes attempts).
    fn run_chrome(&self, cases: Vec<Value>) -> Res<Value> {
        let chrome = self.current_chrome().ok_or("no Chrome in this executor")?;
        let mut records = chrome.run(cases.clone());
        if chrome.is_dead() {
            let host_failed = |record: &Value| record["failure"]["phase"] == "chrome-host";
            let again: Vec<Value> = cases.into_iter().filter(|case| case["id"].as_str().is_some_and(|id| records.get(id).is_some_and(host_failed))).collect();
            if !again.is_empty() {
                if let Some(fresh) = self.restart_chrome(&chrome) {
                    eprintln!("fuzz-exec: {} programs of the batch run again on the new browser", again.len());
                    if let (Value::Object(records), Value::Object(retried)) = (&mut records, fresh.run(again)) {
                        records.extend(retried);
                    }
                }
            }
        }
        Ok(records)
    }

    /// One program's step 1 (pipeline.mjs `prepareEntries`): its files, its type checks, the checker on the clean text.
    fn prepare(&self, request: &Value) -> Value {
        let seq = request["seq"].clone();
        match self.prepare_projects(request) {
            Ok(answer) => {
                let mut answer = answer;
                answer["seq"] = seq;
                answer
            }
            Err(error) => json!({ "seq": seq, "error": error }),
        }
    }

    fn prepare_projects(&self, request: &Value) -> Res<Value> {
        for pair in request["caseWrites"].as_array().into_iter().flatten() {
            if let (Some(path), Some(text)) = (pair[0].as_str(), pair[1].as_str()) {
                write_file(path, text)?;
            }
        }
        let project = request["project"].as_object();
        let twin_project = request["twinProject"].as_object();
        // A project with a directory of its own (a program that ships a package, or `--keep-work`) is written fresh there and removed
        // after; the others borrow a slot pair.
        let slotted = project.is_some_and(|p| p.get("dir").is_none()) || twin_project.is_some_and(|p| p.get("dir").is_none());
        let shape = slot_shape(request);
        let mut slot = slotted.then(|| self.slots.take(shape.as_deref()));
        // The program's id names its slot directory (`SlotDir`); a request without one (an older caller) shares the name `program`.
        // A warm checker's slot keeps one name: its daemon is keyed by the project path, so a directory per program would start a
        // daemon per program. The role log then names the program from the check's environment instead (`Checker::command`).
        let program = request["id"].as_str();
        let id = if self.checker.warm == Warm::Off { program.unwrap_or("program") } else { "program" };
        let result = (|| -> Res<Value> {
            let place = |project: &serde_json::Map<String, Value>, slot_dir: Option<&mut SlotDir>| -> Res<PathBuf> {
                let files = project.get("files").and_then(Value::as_object).ok_or("project without files")?;
                match project.get("dir").and_then(Value::as_str) {
                    Some(dir) => {
                        write_project(Path::new(dir), files)?;
                        Ok(PathBuf::from(dir))
                    }
                    None => {
                        let slot_dir = slot_dir.ok_or("no slot")?;
                        let dir = slot_dir.place(id).map_err(|e| format!("slot {}: {e}", slot_dir.parent.display()))?;
                        // Unknown until the files are written: a failed write leaves a slot whose project nothing describes.
                        slot_dir.shape = None;
                        sync_files(&dir, files).map_err(|e| format!("slot {}: {e}", dir.display()))?;
                        slot_dir.shape = Some(project_shape(files));
                        Ok(dir)
                    }
                }
            };
            let dir = project.map(|p| place(p, slot.as_mut().map(|s| &mut s.0))).transpose()?;
            let twin_dir = twin_project.map(|p| place(p, slot.as_mut().map(|s| &mut s.1))).transpose()?;
            let string = |path: &PathBuf| path.to_string_lossy().into_owned();
            let clean = (request["clean"] == true).then(|| dir.as_ref().map(|dir| self.batcher.submit(string(dir)))).flatten();
            let twin = (request["twin"] == true).then(|| twin_dir.as_ref().map(|dir| self.batcher.submit(string(dir)))).flatten();
            let lost = || json!({ "ok": false, "errors": ["type check lost"] });
            let typecheck = clean.map(|answer| answer.recv().unwrap_or_else(|_| lost()));
            if typecheck.as_ref().is_some_and(|answer| answer["ok"] != true) {
                // The twin's check may still be queued on the slot; the slot goes back only once it answered (else the next program on
                // the slot could share its batch and its answer).
                if let Some(answer) = twin {
                    let _ = answer.recv();
                }
                return Ok(json!({ "typecheck": typecheck }));
            }
            let checker = match (request["checker"].as_object(), dir.as_ref()) {
                (Some(check), Some(dir)) => {
                    let ordinary = check.get("ordinary").and_then(Value::as_bool).unwrap_or(false);
                    self.checks.acquire();
                    let slotted = project.is_some_and(|p| p.get("dir").is_none());
                    let answer = self.checker.run(&string(dir), ordinary, slotted, program);
                    self.checks.release();
                    Some(answer)
                }
                _ => None,
            };
            let twin_typecheck = twin.map(|answer| answer.recv().unwrap_or_else(|_| lost()));
            Ok(json!({ "typecheck": typecheck, "twinTypecheck": twin_typecheck, "checker": checker }))
        })();
        if let Some(slot) = slot {
            self.slots.give(slot);
        }
        if !self.keep_work {
            for project in [project, twin_project].into_iter().flatten() {
                if let Some(dir) = project.get("dir").and_then(Value::as_str) {
                    let _ = std::fs::remove_dir_all(dir);
                }
            }
        }
        result
    }
}

fn main() {
    let out = Arc::new(Out(Mutex::new(std::io::stdout())));
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let executor = match lines.next() {
        Some(Ok(line)) => {
            let request: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
            match Executor::init(Arc::clone(&out), &request) {
                Ok(executor) => {
                    out.emit(&json!({ "op": "ready", "chrome": executor.current_chrome().map(|chrome| chrome.base.clone()) }));
                    Arc::new(executor)
                }
                Err(error) => {
                    out.emit(&json!({ "op": "ready", "error": error }));
                    return;
                }
            }
        }
        _ => return,
    };
    // Step-1 work runs on a pool twice the concurrency (a program waiting for its batch holds no checker), Chrome runs on their own.
    let workers = executor.checks.capacity() * 2;
    let jobs = Arc::new(Jobs::new(workers * 4));
    for _ in 0..workers {
        let executor = Arc::clone(&executor);
        let jobs = Arc::clone(&jobs);
        thread::spawn(move || {
            while let Some(job) = jobs.next(|| executor.slots.free_shapes()) {
                let answer = executor.prepare(&job);
                executor.out.emit(&answer);
            }
        });
    }
    for line in lines {
        let Ok(line) = line else { break };
        let Ok(request) = serde_json::from_str::<Value>(&line) else { continue };
        match request["op"].as_str() {
            Some("prepare") => jobs.push(request),
            // A new number of concurrent checker runs (matrix.mjs: with every runtime cached, the idle Chrome share goes to the
            // checker). Capped at the step-1 pool, which is twice the starting concurrency.
            Some("checks") => {
                let count = request["count"].as_u64().unwrap_or(1) as usize;
                executor.checks.resize(count.min(workers));
                executor.out.emit(&json!({ "seq": request["seq"].clone(), "checks": executor.checks.capacity() }));
            }
            Some("run") => {
                let executor = Arc::clone(&executor);
                thread::spawn(move || {
                    let seq = request["seq"].clone();
                    let cases = request["cases"].as_array().cloned().unwrap_or_default();
                    match executor.run_chrome(cases) {
                        Ok(records) => executor.out.emit(&json!({ "seq": seq, "records": records })),
                        Err(error) => executor.out.emit(&json!({ "seq": seq, "error": error })),
                    }
                });
            }
            _ => {}
        }
    }
    // stdin closed: the run is over (as the request channel's end did before: the workers end on an empty queue). The browser and the
    // Vite helper are killed and waited on, and every slot's checker daemon is stopped.
    jobs.close();
    if let Some(chrome) = executor.current_chrome() {
        chrome.stop();
    }
    executor.checker.stop_warm();
}

#[cfg(test)]
mod tests {
    use super::typecheck::{batch_answers, has_ts_error, resolve_lexical};
    use super::*;

    #[test]
    fn a_command_answers_its_exit_code_and_on_linux_its_resource_use() {
        let mut command = Command::new("sh");
        command.args(["-c", "exit 3"]);
        let ran = run_command(command, Duration::from_secs(10));
        assert_eq!(ran.status, Some(3));
        assert_eq!(ran.usage.is_some(), cfg!(target_os = "linux"));
        if let Some(usage) = ran.usage {
            assert!(usage.max_rss_kb > 0);
        }
        // A signal leaves no code, as `ExitStatus::code` says.
        let mut command = Command::new("sh");
        command.args(["-c", "kill -9 $$"]);
        assert_eq!(run_command(command, Duration::from_secs(10)).status, None);
        let fields = usage_fields(&run_command(Command::new("true"), Duration::from_secs(10)), true);
        assert_eq!(fields["usageWhy"], "warm daemon");
        assert!(fields["maxRssKb"].is_null());
    }

    #[test]
    fn a_semaphore_grows_and_shrinks_under_its_holders() {
        let semaphore = Arc::new(Semaphore::new(1));
        semaphore.acquire();
        semaphore.resize(3);
        semaphore.acquire();
        semaphore.acquire();
        assert_eq!(*semaphore.free.lock().unwrap(), 0);
        // Shrunk below its three holders: a fourth waits until two of them release.
        semaphore.resize(1);
        assert_eq!(*semaphore.free.lock().unwrap(), -2);
        let waiting = {
            let semaphore = Arc::clone(&semaphore);
            thread::spawn(move || semaphore.acquire())
        };
        semaphore.release();
        semaphore.release();
        thread::sleep(Duration::from_millis(50));
        assert!(!waiting.is_finished());
        semaphore.release();
        waiting.join().unwrap();
        assert_eq!(semaphore.capacity(), 1);
        assert_eq!(*semaphore.free.lock().unwrap(), 0);
    }

    /// A DevTools endpoint that accepts one WebSocket, answers its first command, then closes: what a browser that dies looks like.
    fn dying_endpoint() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            if let Ok(tungstenite::Message::Text(text)) = socket.read() {
                let id = serde_json::from_str::<Value>(&text).unwrap()["id"].clone();
                let _ = socket.send(tungstenite::Message::Text(json!({ "id": id, "result": { "ok": true } }).to_string().into()));
            }
            thread::sleep(Duration::from_millis(200));
            let _ = socket.close(None);
        });
        format!("ws://{address}/devtools/browser/test")
    }

    #[test]
    fn a_closed_connection_fails_waiting_and_later_commands_at_once() {
        let cdp = Cdp::connect(&dying_endpoint()).unwrap();
        assert_eq!(cdp.call(None, "Browser.getVersion", json!({})).unwrap()["ok"], true);
        // The peer closes while this call waits: it fails when the connection does, not after the 30 s call deadline.
        let started = Instant::now();
        assert!(cdp.call(None, "Target.createTarget", json!({})).is_err());
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(cdp.is_closed());
        let started = Instant::now();
        let error = cdp.call(None, "Target.createTarget", json!({})).unwrap_err();
        assert!(error.contains("the browser is gone"), "{error}");
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn a_child_that_exits_is_reaped_and_marks_the_browser_dead() {
        let cdp = Cdp::connect(&dying_endpoint()).unwrap();
        let child = Arc::new(Mutex::new(Command::new("sh").arg("-c").arg("echo bye >&2; exit 3").stderr(Stdio::null()).spawn().unwrap()));
        let dead = Arc::new(AtomicBool::new(false));
        supervise("a test child", Arc::clone(&child), None, Arc::clone(&cdp), Arc::clone(&dead), Arc::new(AtomicBool::new(false)));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !dead.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        assert!(dead.load(Ordering::SeqCst));
        assert!(cdp.is_closed());
        // Reaped: waiting again returns its status at once.
        assert_eq!(child.lock().unwrap().try_wait().unwrap().and_then(|status| status.code()), Some(3));
    }

    #[test]
    fn a_startup_line_that_never_comes_fails_at_its_deadline() {
        let mut child = Command::new("sleep").arg("5").stdout(Stdio::piped()).spawn().unwrap();
        let started = Instant::now();
        let error = read_line_within(child.stdout.take().unwrap(), Duration::from_millis(300)).unwrap_err();
        assert!(error.contains("not ready"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(2));
        let _ = child.kill();
        let _ = child.wait();
        let mut quiet = Command::new("true").stdout(Stdio::piped()).spawn().unwrap();
        assert!(read_line_within(quiet.stdout.take().unwrap(), Duration::from_secs(2)).unwrap_err().contains("exited"));
        let _ = quiet.wait();
    }

    fn dirs() -> Vec<String> {
        vec!["/w/a".into(), "/w/b".into()]
    }

    #[test]
    fn a_batch_gives_each_project_its_errors_as_a_lone_run_prints_them() {
        let out = "../b/App.tsx(3,7): error TS2322: Type 'number' is not assignable to type 'string'.\nnoise\n";
        let answers = batch_answers(&dirs(), out, Some(2), false).unwrap();
        assert_eq!(answers[0], json!({ "ok": true, "errors": [] }));
        assert_eq!(answers[1], json!({ "ok": false, "errors": ["App.tsx(3,7): error TS2322: Type 'number' is not assignable to type 'string'."] }));
    }

    #[test]
    fn a_batch_answers_nothing_when_it_crashed_timed_out_or_names_another_file() {
        assert!(batch_answers(&dirs(), "", Some(1), false).is_none(), "a failure with no diagnostic");
        assert!(batch_answers(&dirs(), "", Some(0), true).is_none(), "killed at its timeout");
        assert!(batch_answers(&dirs(), "../c/App.tsx(1,1): error TS1: x", Some(2), false).is_none(), "a file of no project");
        assert!(batch_answers(&dirs(), "App.tsx(x,1): error TS1: x", Some(2), false).is_none(), "an unparsable position");
        assert_eq!(batch_answers(&dirs(), "", Some(0), false).unwrap().len(), 2);
    }

    #[test]
    fn a_slot_holds_exactly_the_program_files_and_keeps_an_equal_file_untouched() {
        let dir = std::env::temp_dir().join(format!("fuzz-exec-slot-test-{}", std::process::id()));
        let files = |pairs: &[(&str, &str)]| pairs.iter().map(|(name, text)| (name.to_string(), json!(text))).collect::<serde_json::Map<_, _>>();
        sync_files(&dir, &files(&[("App.tsx", "a"), ("helpers.ts", "h"), ("main.tsx", "m")])).unwrap();
        std::fs::write(dir.join("tsconfig.tsbuildinfo"), "stale").unwrap();
        let main = std::fs::metadata(dir.join("main.tsx")).unwrap().modified().unwrap();
        sync_files(&dir, &files(&[("App.tsx", "b"), ("main.tsx", "m")])).unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["App.tsx", "main.tsx"]);
        assert_eq!(std::fs::read_to_string(dir.join("App.tsx")).unwrap(), "b");
        assert_eq!(std::fs::metadata(dir.join("main.tsx")).unwrap().modified().unwrap(), main);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_check_runs_its_own_fan_outs_on_one_worker_unless_told_otherwise() {
        let env_of = |checker: &Checker| {
            let command = checker.command("/tmp/x", &[], true, None);
            command.get_envs().map(|(key, value)| (key.to_string_lossy().into_owned(), value.map(|v| v.to_string_lossy().into_owned()))).collect::<HashMap<_, _>>()
        };
        let set = |pairs: &[(&str, &str)]| Checker { bin: "checker".into(), env: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), warm: Warm::On, warmed: Mutex::new(Default::default()) };
        let env = env_of(&set(&[]));
        for (key, value) in CHECK_WORKERS {
            if std::env::var_os(key).is_none() {
                assert_eq!(env.get(key), Some(&Some(value.to_string())), "{key}");
            }
        }
        let env = env_of(&set(&[("RAYON_NUM_THREADS", "4")]));
        assert_eq!(env.get("RAYON_NUM_THREADS"), Some(&Some("4".to_string())));
    }

    #[test]
    fn a_project_takes_a_free_slot_that_held_its_shape_first() {
        let files = |names: &[&str], tsconfig: &str| {
            names.iter().map(|name| (name.to_string(), json!(if *name == "tsconfig.json" { tsconfig } else { "x" }))).collect::<serde_json::Map<_, _>>()
        };
        let plain = project_shape(&files(&["App.tsx", "main.tsx", "tsconfig.json"], "a"));
        let helper = project_shape(&files(&["App.tsx", "helpers.ts", "main.tsx", "tsconfig.json"], "b"));
        // Contents other than the tsconfig do not move the shape; names and the tsconfig do.
        let mut edited = files(&["App.tsx", "main.tsx", "tsconfig.json"], "a");
        edited.insert("App.tsx".into(), json!("y"));
        assert_eq!(project_shape(&edited), plain);
        assert_ne!(project_shape(&files(&["App.tsx", "main.tsx", "tsconfig.json"], "c")), plain);
        assert_ne!(helper, plain);
        let slot = |n: &str, shape: Option<&str>| {
            let mut dir = SlotDir::new(PathBuf::from(n));
            dir.shape = shape.map(str::to_owned);
            (dir, SlotDir::new(PathBuf::from(format!("{n}-run"))))
        };
        let slots = Slots { free: Mutex::new(vec![slot("0", Some(&plain)), slot("1", Some(&helper)), slot("2", None)]), ready: Condvar::new() };
        assert_eq!(slots.take(Some(&plain)).0.parent, PathBuf::from("0"));
        // No free slot of the shape: the last freed one, as before.
        assert_eq!(slots.take(Some(&plain)).0.parent, PathBuf::from("2"));
        assert_eq!(slots.take(None).0.parent, PathBuf::from("1"));
    }

    #[test]
    fn a_worker_takes_the_request_a_free_slot_has_the_shape_of_and_passes_none_over_for_ever() {
        let request = |id: u64, shape: &str| json!({ "seq": id, "project": { "files": { "App.tsx": "x", shape: "y", "tsconfig.json": "t" } } });
        let shape = |name: &str| slot_shape(&request(0, name)).unwrap();
        let seq = |job: Option<Value>| job.unwrap()["seq"].as_u64().unwrap();
        let jobs = Jobs::new(2);
        for (id, name) in [(1, "a.ts"), (2, "b.ts"), (3, "b.ts"), (4, "b.ts"), (5, "a.ts")] {
            jobs.push(request(id, name));
        }
        // A project with a directory of its own takes no slot and has no shape.
        assert_eq!(slot_shape(&json!({ "project": { "dir": "/x", "files": { "App.tsx": "x" } } })), None);
        assert_eq!(seq(jobs.next(|| vec![shape("b.ts")])), 2);
        assert_eq!(seq(jobs.next(|| vec![])), 1);
        // 5 is taken past 3 and 4 while their bound lasts; then the oldest goes first whatever the slots hold.
        assert_eq!(seq(jobs.next(|| vec![shape("a.ts")])), 5);
        jobs.push(request(6, "a.ts"));
        jobs.push(request(7, "a.ts"));
        assert_eq!(seq(jobs.next(|| vec![shape("a.ts")])), 6);
        assert_eq!(seq(jobs.next(|| vec![shape("a.ts")])), 3);
        assert_eq!(seq(jobs.next(|| vec![shape("a.ts")])), 4);
        assert_eq!(seq(jobs.next(|| vec![shape("a.ts")])), 7);
        jobs.close();
        assert!(jobs.next(Vec::new).is_none());
    }

    #[test]
    fn a_slot_directory_is_named_for_its_program_and_keeps_its_files_across_programs() {
        let root = std::env::temp_dir().join(format!("fuzz-exec-slot-name-test-{}", std::process::id()));
        let mut slot = SlotDir::new(root.join("0"));
        let files = |text: &str| [("App.tsx".to_string(), json!(text)), ("main.tsx".to_string(), json!("m"))].into_iter().collect::<serde_json::Map<_, _>>();
        let first = slot.place("lg-a-closure").unwrap();
        assert_eq!(first, root.join("0/lg-a-closure"));
        sync_files(&first, &files("a")).unwrap();
        let main = std::fs::metadata(first.join("main.tsx")).unwrap().modified().unwrap();
        let second = slot.place("lg-a-closure-helper").unwrap();
        assert_eq!(second, root.join("0/lg-a-closure-helper"));
        assert!(!first.exists(), "the previous program's directory is renamed, not kept");
        sync_files(&second, &files("b")).unwrap();
        assert_eq!(std::fs::metadata(second.join("main.tsx")).unwrap().modified().unwrap(), main, "an equal file survives the rename untouched");
        assert_eq!(slot.place("lg-a-closure-helper").unwrap(), second);
        let names: Vec<String> = std::fs::read_dir(root.join("0")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["lg-a-closure-helper"]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_slot_name_is_one_directory_name_per_id() {
        assert_eq!(slot_name("mx-flush-in-effect-callback-effect-apply-direct-none-m-show"), "mx-flush-in-effect-callback-effect-apply-direct-none-m-show");
        assert_eq!(slot_name("a/b c"), "a%2Fb%20c");
        assert_eq!(slot_name(".."), "%2E.");
        assert_eq!(slot_name(""), "%00");
        let long = "x".repeat(300);
        let name = slot_name(&long);
        assert!(name.len() <= 200 && name.starts_with(&"x".repeat(160)), "{name}");
        assert_ne!(name, slot_name(&format!("{long}y")));
        let encoded = slot_name(&"/".repeat(100));
        assert!(encoded.len() <= 200 && !encoded.split('~').next().unwrap().ends_with('%'), "{encoded}");
    }

    #[test]
    fn an_error_line_is_one_that_names_a_ts_code() {
        assert!(has_ts_error("App.tsx(1,1): error TS2322: x"));
        assert!(!has_ts_error("error TSX"));
        assert_eq!(resolve_lexical(Path::new("/w/a"), "../b/./App.tsx"), PathBuf::from("/w/b/App.tsx"));
    }

    /// A stand-in checker: logs `<SOLID_CHECKER_DAEMON> <args>` (and `program=<id>` when the role log's program is set) and prints
    /// what its environment says, or exits 9 when told to.
    fn fake_checker(dir: &Path) -> (String, PathBuf) {
        let log = dir.join("calls.log");
        let bin = dir.join("fake-checker");
        std::fs::write(
            &bin,
            format!(
                "#!/bin/sh\necho \"$SOLID_CHECKER_DAEMON $*\" >> {0}\n[ -n \"$SOLID_CHECKER_IR_ROLE_LOG_PROGRAM\" ] && echo \"program=$SOLID_CHECKER_IR_ROLE_LOG_PROGRAM\" >> {0}\ncase \"$*\" in *--stop-daemon*) exit 0;; esac\n[ \"$FAKE_FAIL$SOLID_CHECKER_DAEMON\" = 11 ] && exit 9\nprintf '{{\"daemon\":%s,\"diverge\":%s}}' \"$SOLID_CHECKER_DAEMON\" \"${{FAKE_DIVERGE:-0}}\"\n",
                log.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&bin).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&bin, permissions).unwrap();
        (bin.to_string_lossy().into_owned(), log)
    }

    fn checker(bin: &str, warm: Warm, env: &[(&str, &str)]) -> Checker {
        Checker { bin: bin.into(), env: env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), warm, warmed: Mutex::new(Default::default()) }
    }

    #[test]
    fn a_warm_checker_serves_slots_only_and_stops_their_daemons() {
        let dir = std::env::temp_dir().join(format!("fuzz-exec-warm-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (bin, log) = fake_checker(&dir);
        let calls = || std::fs::read_to_string(&log).unwrap_or_default();
        let slot = dir.to_string_lossy().into_owned();

        // Off: one-shot everywhere.
        let answer = checker(&bin, Warm::Off, &[]).run(&slot, false, true, None);
        assert_eq!(answer["stdout"], "{\"daemon\":0,\"diverge\":0}");
        assert!(answer.get("warm").is_none());
        assert!(!calls().contains("program="));

        // On: a slot's feedback check goes to the daemon; an ordinary check and a project with its own directory do not.
        std::fs::remove_file(&log).unwrap();
        let warm = checker(&bin, Warm::On, &[]);
        assert_eq!(warm.run(&slot, false, true, Some("fuzz-3"))["stdout"], "{\"daemon\":1,\"diverge\":0}");
        assert_eq!(warm.run(&slot, true, true, None)["stdout"], "{\"daemon\":0,\"diverge\":0}");
        assert_eq!(warm.run(&slot, false, false, None)["stdout"], "{\"daemon\":0,\"diverge\":0}");
        assert!(calls().lines().next().unwrap().starts_with("1 --project"));
        // The role log's program goes with the check that named it, warm or not.
        assert_eq!(calls().lines().nth(1), Some("program=fuzz-3"));
        assert_eq!(calls().matches("program=").count(), 1);
        // The executor's end stops the daemon of each slot a warm check used, once, with the check's own mode flags.
        warm.stop_warm();
        let stops: Vec<String> = calls().lines().filter(|line| line.contains("--stop-daemon")).map(str::to_string).collect();
        assert_eq!(stops.len(), 1);
        assert!(stops[0].contains("--feedback-facts"), "{}", stops[0]);
        warm.stop_warm();
        assert_eq!(calls().lines().filter(|line| line.contains("--stop-daemon")).count(), 1);

        // A warm check that fails stops its daemon before the slot is reused.
        std::fs::remove_file(&log).unwrap();
        let failing = checker(&bin, Warm::On, &[("FAKE_FAIL", "1")]);
        assert_eq!(failing.run(&slot, false, true, None)["status"], 9);
        assert!(calls().lines().last().unwrap().contains("--stop-daemon"));

        // Verify: the one-shot run at the same path follows, and the two are compared.
        let verify = checker(&bin, Warm::Verify, &[]);
        let answer = verify.run(&slot, false, true, Some("fuzz-4"));
        assert_eq!(answer["warm"]["identical"], false, "the stand-in prints its mode, so the outputs differ");
        assert_eq!(answer["warm"]["oneShotStatus"], 0);
        // Both runs of a verified check name its program.
        assert_eq!(calls().matches("program=fuzz-4").count(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}