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
//!   init     the tools, the concurrency, and (optional) the Chrome settings; answers `ready`
//!   prepare  the case's files, its clean and twin projects (in a fixed slot pair, or given directories), the type checks and the
//!            checker run; answers the type checks and the checker's raw output
//!   run      cases to run in Chrome; answers their records by id
//! Parsing the checker's output and every verdict stay in JS: this process returns what the tools printed.

use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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

struct Semaphore {
    free: Mutex<usize>,
    ready: Condvar,
}
impl Semaphore {
    fn new(count: usize) -> Self {
        Self { free: Mutex::new(count.max(1)), ready: Condvar::new() }
    }
    fn acquire(&self) {
        let mut free = self.free.lock().unwrap();
        while *free == 0 {
            free = self.ready.wait(free).unwrap();
        }
        *free -= 1;
    }
    fn release(&self) {
        *self.free.lock().unwrap() += 1;
        self.ready.notify_one();
    }
}

struct Ran {
    status: Option<i32>,
    stdout: String,
    stderr: String,
    killed: bool,
    wall_ms: u128,
    spawn_error: Option<String>,
}

/// Runs a command to completion with its outputs read on threads, killed after `timeout`.
fn run_command(mut command: Command, timeout: Duration) -> Ran {
    let started = Instant::now();
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Ran { status: None, stdout: String::new(), stderr: String::new(), killed: false, wall_ms: 0, spawn_error: Some(error.to_string()) };
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
    let status = child.wait().ok().and_then(|status| status.code());
    let _ = finished.send(());
    let killed = watchdog.join().unwrap_or(false);
    let stdout = String::from_utf8_lossy(&out.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&err.join().unwrap_or_default()).into_owned();
    Ran { status, stdout, stderr, killed, wall_ms: started.elapsed().as_millis(), spawn_error: None }
}

fn write_file(path: &str, text: &str) -> Res<()> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

/// `path.resolve(base, relative)` without touching the disk.
fn resolve_lexical(base: &Path, relative: &str) -> PathBuf {
    let joined = base.join(relative);
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Type checks (scripts/fuzz/typecheck.mjs: `typecheck` and `typecheckBatch`, the same answers)

fn has_ts_error(line: &str) -> bool {
    line.find("error TS").is_some_and(|at| line[at + 8..].starts_with(|c: char| c.is_ascii_digit()))
}

/// `typecheck(dir)`: one `tsrs -p`.
fn typecheck_one(tsc: &str, dir: &str) -> Value {
    if !Path::new(tsc).exists() {
        return json!({ "ok": false, "errors": [format!("type checker not found: {tsc}")], "missing": true });
    }
    let mut command = Command::new(tsc);
    command.args(["-p", &format!("{dir}/tsconfig.json"), "--noEmit"]).current_dir(dir);
    let ran = run_command(command, Duration::from_secs(60));
    if let Some(error) = ran.spawn_error {
        return json!({ "ok": false, "errors": [error], "missing": true });
    }
    let out = format!("{}{}", ran.stdout, ran.stderr);
    let errors: Vec<&str> = out.split('\n').filter(|line| has_ts_error(line)).collect();
    json!({ "ok": ran.status == Some(0) && errors.is_empty(), "errors": errors })
}

/// `typecheckBatch(dirs)`: one `tsrs -b --force`; `None` where the batch gives no answer (the caller checks each alone).
fn typecheck_batch(tsc: &str, dirs: &[String]) -> Option<Vec<Value>> {
    if !Path::new(tsc).exists() || dirs.is_empty() {
        return None;
    }
    let mut command = Command::new(tsc);
    command.arg("-b").arg("--force");
    for dir in dirs {
        command.arg(format!("{dir}/tsconfig.json"));
    }
    command.arg("--noEmit").current_dir(&dirs[0]);
    let ran = run_command(command, Duration::from_secs(120));
    for dir in dirs {
        let _ = std::fs::remove_file(format!("{dir}/tsconfig.tsbuildinfo"));
    }
    if ran.spawn_error.is_some() {
        return None;
    }
    batch_answers(dirs, &format!("{}{}", ran.stdout, ran.stderr), ran.status, ran.killed)
}

/// A batch's output as one answer per project, or `None` (typecheck.mjs `typecheckBatch`'s rules).
fn batch_answers(dirs: &[String], out: &str, status: Option<i32>, killed: bool) -> Option<Vec<Value>> {
    let lines: Vec<&str> = out.split('\n').filter(|line| has_ts_error(line)).collect();
    if killed || (status != Some(0) && lines.is_empty()) {
        return None;
    }
    let mut answers: Vec<(bool, Vec<String>)> = dirs.iter().map(|_| (true, Vec::new())).collect();
    let base = Path::new(&dirs[0]);
    for line in lines {
        // `^(.*?)\(\d+,\d+\): error TS\d+`
        let at = line.find("): error TS")?;
        let open = line[..at].rfind('(')?;
        let numbers = &line[open + 1..at];
        let mut parts = numbers.split(',');
        let valid = parts.next().is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            && parts.next().is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            && parts.next().is_none();
        if !valid {
            return None;
        }
        let file_text = &line[..open];
        let file = resolve_lexical(base, file_text);
        let index = dirs.iter().position(|dir| file.starts_with(dir) && file != Path::new(dir))?;
        let relative = file.strip_prefix(&dirs[index]).ok()?.to_string_lossy().into_owned();
        answers[index].0 = false;
        answers[index].1.push(format!("{relative}{}", &line[file_text.len()..]));
    }
    Some(answers.into_iter().map(|(ok, errors)| json!({ "ok": ok, "errors": errors })).collect())
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

struct Checker {
    bin: String,
    env: Vec<(String, String)>,
}

impl Checker {
    fn run(&self, dir: &str, ordinary: bool) -> Value {
        if !Path::new(&self.bin).exists() {
            return json!({ "spawnError": format!("checker binary not found: {}", self.bin), "dir": dir });
        }
        let mut command = Command::new(&self.bin);
        command.arg("--project").arg(format!("{dir}/tsconfig.json")).args(if ordinary { &ORDINARY_ARGS[..] } else { &CHECKER_ARGS[..] }).current_dir(dir);
        for (key, value) in &self.env {
            command.env(key, value);
        }
        command.env("SOLID_CHECKER_DAEMON", "0");
        let ran = run_command(command, Duration::from_secs(180));
        if let Some(error) = ran.spawn_error {
            return json!({ "spawnError": error, "dir": dir, "wallMs": ran.wall_ms as u64 });
        }
        json!({ "status": ran.status, "stdout": ran.stdout, "stderr": ran.stderr, "wallMs": ran.wall_ms as u64, "dir": dir })
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
}

impl Cdp {
    fn connect(url: &str) -> Res<Arc<Cdp>> {
        let address = url.trim_start_matches("ws://").split('/').next().ok_or("bad DevTools URL")?.to_string();
        let stream = TcpStream::connect(&address).map_err(|e| format!("connect {address}: {e}"))?;
        stream.set_nodelay(true).ok();
        let writer_stream = stream.try_clone().map_err(|e| e.to_string())?;
        let (mut socket, _) = tungstenite::client(url, stream).map_err(|e| format!("DevTools handshake: {e}"))?;
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
        let cdp = Arc::new(Cdp { outgoing: Mutex::new(outgoing), next: AtomicU64::new(1), pending: Mutex::new(HashMap::new()), sessions: Mutex::new(HashMap::new()) });
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
                        router.pending.lock().unwrap().clear();
                        router.sessions.lock().unwrap().clear();
                        return;
                    }
                }
            }
        });
        Ok(cdp)
    }

    /// Sends a command; its response goes to `reply`. Returns the command's id.
    fn send(&self, session: Option<&str>, method: &str, params: Value, reply: Sender<Value>) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.pending.lock().unwrap().insert(id, reply);
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = json!(session);
        }
        let _ = self.outgoing.lock().unwrap().send(message.to_string());
        id
    }

    fn call(&self, session: Option<&str>, method: &str, params: Value) -> Res<Value> {
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

struct Chrome {
    cdp: Arc<Cdp>,
    base: String,
    control: String,
    work: PathBuf,
    runs: AtomicU64,
    pages: usize,
    idle: Mutex<Vec<Page>>,
    font_families: Value,
    _browser: Mutex<std::process::Child>,
    _vite: Mutex<std::process::Child>,
    _profile: PathBuf,
}

struct Page {
    context: String,
    session: String,
    frame: String,
    events: Receiver<Value>,
    reply: Sender<Value>,
}

fn http_post(base: &str, path: &str, body: &str) -> Res<String> {
    let address = base.trim_start_matches("http://").trim_end_matches('/').to_string();
    let mut stream = TcpStream::connect(&address).map_err(|e| format!("vite helper {address}: {e}"))?;
    write!(stream, "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&response).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").ok_or("vite helper: malformed response")?;
    if !head.starts_with("HTTP/1.1 200") {
        return Err(format!("vite helper {path}: {}", head.lines().next().unwrap_or("")));
    }
    Ok(body.to_string())
}

impl Chrome {
    fn start(settings: &Value) -> Res<Chrome> {
        let text = |key: &str| settings.get(key).and_then(Value::as_str).map(str::to_string).ok_or(format!("chrome.{key} missing"));
        // The Vite helper: one line `{ base, work }` once it serves.
        let mut vite = Command::new(text("node")?)
            .arg(text("viteHost")?)
            .arg(settings.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("vite helper: {e}"))?;
        let mut line = String::new();
        BufReader::new(vite.stdout.take().unwrap()).read_line(&mut line).map_err(|e| e.to_string())?;
        let ready: Value = serde_json::from_str(line.trim()).map_err(|e| format!("vite helper said {line:?}: {e}"))?;
        let base = ready["base"].as_str().ok_or("vite helper: no base")?.to_string();
        let work = PathBuf::from(ready["work"].as_str().ok_or("vite helper: no work")?);
        let control = ready["control"].as_str().ok_or("vite helper: no control")?.to_string();

        let profile = std::env::temp_dir().join(format!("fuzz-exec-profile-{}", std::process::id()));
        let mut browser = Command::new(text("browser")?)
            .args(BROWSER_ARGS)
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("--remote-debugging-port=0")
            .arg("--no-startup-window")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("browser: {e}"))?;
        let mut stderr = BufReader::new(browser.stderr.take().unwrap());
        let url = loop {
            let mut line = String::new();
            if stderr.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err("the browser exited before it listened".into());
            }
            if let Some(at) = line.find("ws://") {
                break line[at..].trim().to_string();
            }
        };
        thread::spawn(move || {
            let mut sink = String::new();
            while stderr.read_line(&mut sink).is_ok_and(|n| n > 0) {
                sink.clear();
            }
        });
        let cdp = Cdp::connect(&url)?;
        let chrome = Chrome {
            cdp,
            base,
            control,
            work,
            runs: AtomicU64::new(0),
            pages: settings.get("pages").and_then(Value::as_u64).unwrap_or(16) as usize,
            idle: Mutex::new(Vec::new()),
            font_families: settings.get("fontFamilies").cloned().unwrap_or(Value::Null),
            _browser: Mutex::new(browser),
            _vite: Mutex::new(vite),
            _profile: profile,
        };
        // Warm the dependency bundles and the runner once, then freeze the runner as transformed (chrome-host.mjs `start`).
        let page = chrome.open_page()?;
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
        warm?;
        http_post(&chrome.control, "/__sc/freeze", "{}")?;
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
        let deadline = Instant::now() + PAGE_TIMEOUT;
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
            let message = match page.events.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(message) => message,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!("no record within {} ms (committed {committed}, loaded {loaded}, record asked {}, virtual time granted {granted} ms)", PAGE_TIMEOUT.as_millis(), evaluate.is_some()));
                }
                Err(RecvTimeoutError::Disconnected) => return Err("the browser closed".into()),
            };
            match message.get("method").and_then(Value::as_str) {
                Some("Emulation.virtualTimeBudgetExpired") => grant(&mut granted, committed),
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
    free: Mutex<Vec<(PathBuf, PathBuf)>>,
    ready: Condvar,
}
impl Slots {
    fn take(&self) -> (PathBuf, PathBuf) {
        let mut free = self.free.lock().unwrap();
        loop {
            if let Some(pair) = free.pop() {
                return pair;
            }
            free = self.ready.wait(free).unwrap();
        }
    }
    fn give(&self, pair: (PathBuf, PathBuf)) {
        self.free.lock().unwrap().push(pair);
        self.ready.notify_one();
    }
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
    chrome: Option<Arc<Chrome>>,
}

impl Executor {
    fn init(out: Arc<Out>, request: &Value) -> Res<Executor> {
        let text = |key: &str| request.get(key).and_then(Value::as_str).map(str::to_string).ok_or(format!("init.{key} missing"));
        let concurrency = request.get("concurrency").and_then(Value::as_u64).unwrap_or(8) as usize;
        let tsc = text("tsc")?;
        let batcher = Arc::new(Batcher { queue: Mutex::new(Vec::new()), ready: Condvar::new() });
        let batch = request.get("typecheckBatch").and_then(Value::as_u64).unwrap_or(24) as usize;
        batcher.start(tsc.clone(), batch, Duration::from_millis(15), Arc::new(Semaphore::new(concurrency)));
        let env = request.get("env").and_then(Value::as_object).map(|env| env.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string()))).collect()).unwrap_or_default();
        let checker = Arc::new(Checker { bin: text("checkerBin")?, env });
        let slot_root = PathBuf::from(text("slotRoot")?);
        let slots = Slots {
            free: Mutex::new((0..concurrency * 2).map(|slot| (slot_root.join(format!("slot-{slot}")), slot_root.join(format!("slot-{slot}-run")))).collect()),
            ready: Condvar::new(),
        };
        let chrome = match request.get("chrome") {
            Some(settings) if !settings.is_null() => Some(Arc::new(Chrome::start(settings)?)),
            _ => None,
        };
        Ok(Executor {
            out,
            slots,
            batcher,
            checker,
            checks: Arc::new(Semaphore::new(concurrency)),
            keep_work: request.get("keepWork").and_then(Value::as_bool).unwrap_or(false),
            chrome,
        })
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
        let slot = slotted.then(|| self.slots.take());
        let result = (|| -> Res<Value> {
            let place = |project: &serde_json::Map<String, Value>, slot_dir: Option<&PathBuf>| -> Res<PathBuf> {
                let files = project.get("files").and_then(Value::as_object).ok_or("project without files")?;
                match project.get("dir").and_then(Value::as_str) {
                    Some(dir) => {
                        write_project(Path::new(dir), files)?;
                        Ok(PathBuf::from(dir))
                    }
                    None => {
                        let dir = slot_dir.ok_or("no slot")?;
                        sync_files(dir, files).map_err(|e| format!("slot {}: {e}", dir.display()))?;
                        Ok(dir.clone())
                    }
                }
            };
            let dir = project.map(|p| place(p, slot.as_ref().map(|s| &s.0))).transpose()?;
            let twin_dir = twin_project.map(|p| place(p, slot.as_ref().map(|s| &s.1))).transpose()?;
            let string = |path: &PathBuf| path.to_string_lossy().into_owned();
            let clean = (request["clean"] == true).then(|| dir.as_ref().map(|dir| self.batcher.submit(string(dir)))).flatten();
            let twin = (request["twin"] == true).then(|| twin_dir.as_ref().map(|dir| self.batcher.submit(string(dir)))).flatten();
            let lost = || json!({ "ok": false, "errors": ["type check lost"] });
            let typecheck = clean.map(|answer| answer.recv().unwrap_or_else(|_| lost()));
            if typecheck.as_ref().is_some_and(|answer| answer["ok"] != true) {
                return Ok(json!({ "typecheck": typecheck }));
            }
            let checker = match (request["checker"].as_object(), dir.as_ref()) {
                (Some(check), Some(dir)) => {
                    let ordinary = check.get("ordinary").and_then(Value::as_bool).unwrap_or(false);
                    self.checks.acquire();
                    let answer = self.checker.run(&string(dir), ordinary);
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
                    out.emit(&json!({ "op": "ready", "chrome": executor.chrome.as_ref().map(|chrome| chrome.base.clone()) }));
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
    let (jobs, queue) = channel::<Value>();
    let queue = Arc::new(Mutex::new(queue));
    let workers = executor.checks.free.lock().map(|free| *free).unwrap_or(8) * 2;
    for _ in 0..workers {
        let executor = Arc::clone(&executor);
        let queue = Arc::clone(&queue);
        thread::spawn(move || loop {
            let job = { queue.lock().unwrap().recv() };
            let Ok(job) = job else { break };
            let answer = executor.prepare(&job);
            executor.out.emit(&answer);
        });
    }
    for line in lines {
        let Ok(line) = line else { break };
        let Ok(request) = serde_json::from_str::<Value>(&line) else { continue };
        match request["op"].as_str() {
            Some("prepare") => {
                let _ = jobs.send(request);
            }
            Some("run") => {
                let executor = Arc::clone(&executor);
                thread::spawn(move || {
                    let seq = request["seq"].clone();
                    let Some(chrome) = executor.chrome.as_ref() else {
                        executor.out.emit(&json!({ "seq": seq, "error": "no Chrome in this executor" }));
                        return;
                    };
                    let cases = request["cases"].as_array().cloned().unwrap_or_default();
                    let records = chrome.run(cases);
                    executor.out.emit(&json!({ "seq": seq, "records": records }));
                });
            }
            _ => {}
        }
    }
    // stdin closed: the run is over. The browser and the Vite helper die with this process's children.
    if let Some(chrome) = executor.chrome.as_ref() {
        let _ = chrome._browser.lock().unwrap().kill();
        let _ = chrome._vite.lock().unwrap().kill();
        let _ = std::fs::remove_dir_all(&chrome._profile);
        let _ = std::fs::remove_dir_all(&chrome.work);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn an_error_line_is_one_that_names_a_ts_code() {
        assert!(has_ts_error("App.tsx(1,1): error TS2322: x"));
        assert!(!has_ts_error("error TSX"));
        assert_eq!(resolve_lexical(Path::new("/w/a"), "../b/./App.tsx"), PathBuf::from("/w/b/App.tsx"));
    }
}
