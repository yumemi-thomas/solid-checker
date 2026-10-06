//! Asks the project's own bundler where each module load goes (ADR 0220).
//!
//! The worker, `packages/cli/scripts/runtime-resolver.mjs`, loads the
//! project's installed Vite and its config, which runs project code. It is
//! used only when the host asks for it (`--runtime-resolution required`).
//! Every outcome the worker cannot attest, and every failure to run it, leaves
//! the occurrence [`RuntimeOutcome::Unknown`]: the table is still attached, so
//! the analysis fails closed rather than falling back to TypeScript's answer.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use solid_facts::ProjectFacts;
use solid_facts::ast::ModuleLoadKind;
use solid_facts::core::Span;
use solid_facts::runtime_resolution::{RuntimeOutcome, RuntimeResolutionIndex};

const PROTOCOL: &str = "solid-checker-runtime-resolution-v1";
const CONFIG_NAMES: &[&str] = &[
    "vite.config.ts",
    "vite.config.mts",
    "vite.config.cts",
    "vite.config.js",
    "vite.config.mjs",
    "vite.config.cjs",
];
const DEADLINE: Duration = Duration::from_secs(120);

/// How one runtime-resolution attempt went.
#[derive(Clone, Debug, Default)]
pub struct RuntimeResolutionMeasurement {
    pub loads: usize,
    pub answered: usize,
    pub status: String,
}

struct Site {
    path: String,
    span: Span,
    text: String,
    kind: &'static str,
}

fn sites(facts: &ProjectFacts) -> Vec<Site> {
    let mut sites = Vec::new();
    for file in &facts.files {
        let path = file.path.as_str();
        let mut push = |span: Span, text: &str, kind: &'static str| {
            sites.push(Site {
                path: path.to_owned(),
                span,
                text: text.to_owned(),
                kind,
            });
        };
        for import in file.ast.imports.iter().filter(|import| !import.type_only) {
            push(import.span, import.module.as_str(), "static-import");
        }
        for export in file.ast.exports.iter().filter(|export| !export.type_only) {
            if let Some(module) = export.module.as_deref() {
                push(export.span, module, "export-from");
            }
        }
        for import in file
            .ast
            .import_equals
            .iter()
            .filter(|import| !import.type_only)
        {
            push(import.span, import.module.as_str(), "import-equals");
        }
        for load in &file.ast.module_loads {
            if let (Some(text), Some(span)) = (load.specifier.as_deref(), load.specifier_span) {
                let kind = match load.kind {
                    ModuleLoadKind::DynamicImport => "dynamic-import",
                    ModuleLoadKind::Require => "require",
                };
                push(span, text, kind);
            }
        }
    }
    sites
}

/// The Vite config that governs `directory`: the nearest one at or above it,
/// up to the repository or workspace root. A monorepo app often runs the
/// root's config (`vite --config vite.config.ts` from the root).
fn vite_config(directory: &Path) -> Option<PathBuf> {
    let mut current = Some(directory);
    while let Some(candidate) = current {
        if let Some(config) = CONFIG_NAMES
            .iter()
            .map(|name| candidate.join(name))
            .find(|path| path.is_file())
        {
            return Some(config);
        }
        if [".git", "pnpm-workspace.yaml"]
            .iter()
            .any(|marker| candidate.join(marker).exists())
        {
            return None;
        }
        current = candidate.parent();
    }
    None
}

/// Resolves every module load of `facts` through the project's bundler. The
/// index is returned in every case; an occurrence without an answer is
/// unknown.
pub fn resolve_runtime_imports(
    facts: &ProjectFacts,
    project_directory: &Path,
) -> (RuntimeResolutionIndex, RuntimeResolutionMeasurement) {
    let sites = sites(facts);
    let mut index = RuntimeResolutionIndex::default();
    let mut measurement = RuntimeResolutionMeasurement {
        loads: sites.len(),
        ..RuntimeResolutionMeasurement::default()
    };
    let Some(config) = vite_config(project_directory) else {
        measurement.status = "no-vite-config".to_owned();
        return (index, measurement);
    };
    let Some(script) = std::env::var_os("SOLID_CHECKER_RUNTIME_RESOLVER") else {
        measurement.status = "no-resolver-script".to_owned();
        return (index, measurement);
    };
    let node = std::env::var_os("SOLID_CHECKER_PROBE_NODE").unwrap_or_else(|| "node".into());
    let root = config.parent().unwrap_or(project_directory);
    let request = serde_json::json!({
        "protocol": PROTOCOL,
        "root": root,
        "configFile": config,
        "mode": "development",
        "loads": sites.iter().enumerate().map(|(id, site)| serde_json::json!({
            "id": id.to_string(),
            "importer": site.path,
            "specifier": site.text,
            "kind": site.kind,
        })).collect::<Vec<_>>(),
    });
    let report = match run_worker(&node, &script, root, &request) {
        Ok(report) => report,
        Err(status) => {
            measurement.status = status;
            return (index, measurement);
        }
    };
    measurement.status = report
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("error")
        .to_owned();
    if measurement.status != "answered" {
        if let Some(failure) = report.get("failure").and_then(serde_json::Value::as_str) {
            measurement.status = format!("error: {failure}");
        }
        return (index, measurement);
    }
    for row in report
        .get("rows")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(site) = row
            .get("id")
            .and_then(serde_json::Value::as_str)
            .and_then(|id| id.parse::<usize>().ok())
            .and_then(|id| sites.get(id))
        else {
            continue;
        };
        let outcome = row.get("outcome");
        let kind = outcome
            .and_then(|outcome| outcome.get("kind"))
            .and_then(serde_json::Value::as_str);
        let text = |key: &str| {
            outcome
                .and_then(|outcome| outcome.get(key))
                .and_then(serde_json::Value::as_str)
                .map(Arc::<str>::from)
        };
        let outcome = match kind {
            Some("file") => match (text("path"), text("physicalPath")) {
                (Some(path), Some(physical_path)) => RuntimeOutcome::File {
                    path,
                    physical_path,
                },
                _ => RuntimeOutcome::Unknown,
            },
            Some("external") => RuntimeOutcome::External,
            Some("builtin") => RuntimeOutcome::Builtin,
            _ => RuntimeOutcome::Unknown,
        };
        if outcome != RuntimeOutcome::Unknown {
            measurement.answered += 1;
        }
        index.insert(site.path.as_str(), site.span, site.text.as_str(), outcome);
    }
    (index, measurement)
}

fn run_worker(
    node: &std::ffi::OsStr,
    script: &std::ffi::OsStr,
    root: &Path,
    request: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut child = Command::new(node)
        .arg(script)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("spawn: {error}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let bytes = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    let writer = std::thread::spawn(move || stdin.write_all(&bytes));
    // The worker runs project code; it is killed at the deadline.
    let id = child.id();
    let finished = Arc::new(AtomicBool::new(false));
    let watchdog = {
        let finished = Arc::clone(&finished);
        std::thread::spawn(move || {
            let step = Duration::from_millis(100);
            let mut waited = Duration::ZERO;
            while waited < DEADLINE {
                if finished.load(Ordering::SeqCst) {
                    return false;
                }
                std::thread::sleep(step);
                waited += step;
            }
            let _ = Command::new("kill").arg("-9").arg(id.to_string()).status();
            true
        })
    };
    let output = child.wait_with_output();
    finished.store(true, Ordering::SeqCst);
    let timed_out = watchdog.join().unwrap_or(false);
    let _ = writer.join();
    if timed_out {
        return Err("timeout".to_owned());
    }
    let output = output.map_err(|error| format!("wait: {error}"))?;
    let line = String::from_utf8_lossy(&output.stdout);
    let line = line.lines().last().unwrap_or_default();
    serde_json::from_str(line).map_err(|error| format!("report: {error}"))
}
