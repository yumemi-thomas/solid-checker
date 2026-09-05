//! ADR 0033: controlled browser execution over a checker-owned Chrome DevTools
//! Protocol pipe.
//!
//! This module is the browser analogue of the Node worker launch in the parent
//! module, and it keeps the parent's discipline wherever the browser allows it
//! and *says so* wherever it does not. Nothing here is a general browser
//! automation client: it speaks exactly the CDP methods one certification
//! launch needs, over descriptors this process created, to a browser whose
//! bytes were hashed against a compiled-in pin first.
//!
//! # Roots of trust
//!
//! * `SOLID_CHECKER_PROBE_BROWSER_SHA256` — the path-ordered tree digest, in
//!   [`super::hash_tree`]'s framing, of the directory holding the headless-shell
//!   executable. The bundle is a directory rather than one file because the
//!   executable loads its siblings at start-up (the ICU data, the V8 context
//!   snapshot, the resource packs, the GPU libraries) and each of them shapes
//!   the realm the recipe runs in. Symlinks and non-regular entries refuse, as
//!   they do in every watched tree.
//! * The Node pin of the parent module, still: the derived bytes the browser
//!   executes are the native strip-only erasure, and pinned Node must reproduce
//!   every one of them ([`verify_node_reproduces_derived_bytes`]) before a
//!   byte is served. The browser transforms nothing.
//!
//! # Module supply and the resolution premise
//!
//! A browser has no package resolver, so the premise `verify_reported_resolution`
//! rests on — an independent interpreter selecting the same file the Type Facts
//! witness read — has no analogue here, and this module does not pretend one.
//! Instead every request the page makes is intercepted at request stage
//! (`Fetch.enable`, pattern `*`). A request whose exact URL is in the served
//! map is answered from memory with the authenticated derived bytes; any other
//! request is failed *and the launch refuses*. After the run the set of URLs
//! requested must equal the served map exactly, so the graph the browser
//! executed is provably the graph the receipt binds, and nothing else was
//! loaded. The policy digest names this as `resolution:checker-exact-url-map…`
//! so a receipt can never be read as Node's resolution claim.
//!
//! Bare specifiers reach the served map through an import map the launcher
//! derives from the plan: the analyzed package's specifier maps to the root
//! module URL, and each authenticated relative edge maps — under a scope that
//! names the importer's exact URL — from the URL the browser would compute for
//! the literal specifier to the target module's exact URL. An import the edge
//! map does not name resolves to an unmapped URL and refuses.
//!
//! # The browser's own service surface (disposition table)
//!
//! | surface | disposition |
//! | --- | --- |
//! | network | **REFUSED at request stage** for every URL outside the served map; `--host-resolver-rules=MAP * ~NOTFOUND` and `connect-src 'none'` behind it |
//! | filesystem reads by the page | **REFUSED** — `file:` is not a served URL; the renderer runs in Chromium's own sandbox, which this module does not verify |
//! | the browser's profile directory | **CARVE-OUT** — `<private>/browser-profile`, a named subtree inside the 0700 workspace whose *contents* are not watched, because the browser writes there on every launch. Its presence is in the watched entry census and nothing the transaction reads lives in it |
//! | storage (`localStorage`, cookies, IndexedDB) | **NOT DENIED** — lands in the carve-out and is discarded with it |
//! | dedicated and shared workers, iframes | **REFUSED** — `worker-src`/`child-src`/`frame-src 'none'`, and any target the page session auto-attaches refuses the launch |
//! | service workers | **REFUSED** — the feature is disabled and its script fetch would be unmapped |
//! | downloads | **REFUSED** — `Browser.setDownloadBehavior deny` |
//! | permissions, prompts | **REFUSED** — `--deny-permission-prompts`; headless |
//! | inline and `blob:`/`data:` scripts | **REFUSED** — the CSP names the synthetic origin and one per-launch nonce for the import map, nothing else |
//! | in-realm intrinsic mutation | **frozen prototypes only**, as in the Node worker |
//! | the browser's helper processes | **own process group, killed on every exit**; not otherwise contained |
//!
//! # Frames
//!
//! The bootstrap runs before the document exists
//! (`Page.addScriptToEvaluateOnNewDocument`), captures the report binding
//! (`Runtime.addBinding`) and every primordial the report path needs, deletes
//! the binding from `window`, freezes `Object`, `Array` and `Function`
//! prototypes, and reports one startup frame. After `DOMContentLoaded` it
//! imports the recipe, hands it the harness, drains, and reports one run frame
//! in the Node worker's frame shape, built from null-prototype records and
//! serialized without consulting `toJSON`. A frame from any other execution
//! context, without this launch's nonce, or beyond the second is a protocol
//! refusal — never a choice of which frame to believe.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::{self, File},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{atomic::Ordering, mpsc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::Digest;

use super::{
    ABSENT_WATCHED_PATH, BoundProbeHarnessIdentity, EXTRA_FRAME_GRACE, HarnessExecution,
    MAX_REPORT_BYTES, PRIVATE_HARNESS_COUNTER, ProbeHarnessConfiguration, ProbeHarnessError,
    RecipeCorpus, STARTUP_BUDGET, WatchedInput, WorkerProcess, create_private_directory,
    dependency_materialization_root, hash_tree, node_version, remove_private_tree,
    requested_conditions, root, set_close_on_exec, set_directory_permissions, spawn_stderr_reader,
    verify_harness_image, verify_node_executable, watch_digest, write_private_file,
};
use crate::{
    EnvironmentIdentity, SandboxIdentity, SandboxKind, ToolIdentity,
    contract_certification::{
        CertificationPlan, TypeFactsProducerPin, controlled_execution::InertModule,
        probe_gates::ProbeGateSchedule,
    },
    runtime_probe_wire::{
        self, DerivedExecutionEdgeRequest, DerivedExecutionModuleRequest, InertExecutionRequest,
    },
    runtime_probes::{ProbeRun, RuntimeProbeEvaluation, evaluate_runtime_probes},
};

/// The bootstrap/frame protocol the browser page and this launcher agree on.
pub(crate) const BROWSER_PROTOCOL: &str = "solid-checker-runtime-probe-browser-v1";
const BROWSER_STARTUP_FORMAT: &str = "solid-checker-probe-browser-startup";
/// Every served URL lives under this origin. It is not resolvable — `.invalid`
/// is reserved — and every request to it is answered from memory or refused.
const SYNTHETIC_ORIGIN: &str = "https://solid-checker.invalid";
const DOCUMENT_PATH: &str = "/probe.html";
const RECIPE_PREFIX: &str = "/recipes/";
const MODULE_PREFIX: &str = "/modules/";
/// The name `Runtime.addBinding` exposes on `window` until the bootstrap
/// captures and deletes it, which happens before any document script.
const REPORT_BINDING: &str = "__solidCheckerProbeReport";
/// The carve-out: the browser's `--user-data-dir`, inside the private
/// workspace, whose contents are deliberately not watched.
const PROFILE_DIRECTORY: &str = "browser-profile";
/// One CDP message may not exceed this; the report frames are bounded by
/// [`MAX_REPORT_BYTES`] separately.
const CDP_MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;
/// Descriptors `--remote-debugging-pipe` reads from and writes to.
const CDP_IN_DESCRIPTOR: i32 = 3;
const CDP_OUT_DESCRIPTOR: i32 = 4;

/// The field vector the browser scheme's policy digest commits to. A sibling of
/// the Node scheme's vector, not a successor: the Node fields describe a
/// resolver the browser does not have, and bumping them would move every Node
/// receipt without changing its meaning.
///
/// `tests::the_probe_browser_sandbox_policy_digest_names_its_carve_out_and_refusals`
/// compares this against a literal copy.
pub(crate) const BROWSER_SANDBOX_POLICY_FIELDS: [&str; 30] = [
    "scheme-version:11",
    "scheme-family:browser-cdp-pipe",
    "profile:chromium-headless-shell-cdp-pipe-esm,explicit-controlled-consumer,ordinary-acceptance-refused",
    "browser:bundle-tree-digest-reasserted-against-build-pin,version-asked-of-pinned-bytes-and-echoed",
    "driver:checker-owned-cdp-client,launcher-owned-descriptors-3-and-4,no-debugging-port,no-npm-driver",
    "transform:pinned-node-strip-only,parser-runtime-token-preservation,all-derived-outputs-reproduced-by-pinned-node,browser-transforms-nothing",
    "module-supply:request-stage-fulfilment-from-authenticated-derived-bytes,synthetic-origin,exact-url-map",
    "resolution:checker-exact-url-map,import-map-from-plan-resolution-and-authenticated-relative-edges,no-interpreter-resolver,no-export-conditions",
    "resolution:unmapped-request-fails-and-refuses-launch",
    "resolution:requested-url-set-must-equal-served-map",
    "enforcement:detect-and-refuse-plus-browser-enforced-denials",
    "private-directory-mode:0700",
    "cwd:private-directory",
    "environment:allowlisted-not-inherited",
    "argv:launcher-built-flags-only",
    "profile-directory:named-carve-out-inside-private-directory,contents-unwatched,presence-watched",
    "csp:default-src-none,script-src-synthetic-origin-plus-import-map-nonce,connect-worker-child-frame-object-src-none,base-uri-none,form-action-none",
    "targets:auto-attached-new-target-refuses-launch",
    "network:refused-at-request-stage,host-resolver-not-found",
    "downloads:denied",
    "permissions:prompts-denied",
    "service-workers:feature-disabled-and-unmapped",
    "storage:not-denied,discarded-with-carve-out",
    "renderer-sandbox:chromium-own,not-verified-here",
    "report:runtime-binding,exactly-one-startup-and-one-run-frame,main-world-context-only",
    "startup-frame:protocol+nonce+browser-version+ready-state-loading",
    "process-group:own-group-killed-on-every-exit",
    "watched:private-directory-entries,browser-bundle,node-executable,type-facts-image,verifier-image",
    "watched-when:before-first-launch,between-launches,every-exit-path",
    "in-realm-intrinsic-mutation:frozen-prototypes-only",
];

pub(crate) fn browser_sandbox_policy_digest() -> Digest {
    root(
        "probe-browser-sandbox-policy",
        BROWSER_SANDBOX_POLICY_FIELDS,
    )
}

/// The bundle identity of one headless-shell executable: the tree digest of
/// its directory, and that directory.
///
/// The executable must be named by its real path and be a regular file; the
/// directory is hashed with [`hash_tree`], which refuses symlinks and anything
/// that is not a regular file. `scripts/probe-browser-identity.mjs` computes
/// the same digest for the build, in the same framing.
pub(crate) fn browser_bundle_digest(
    executable: &Path,
) -> Result<(String, PathBuf), ProbeHarnessError> {
    let metadata = fs::symlink_metadata(executable).map_err(|error| {
        ProbeHarnessError::Configuration(format!(
            "could not inspect the pinned browser executable: {error}"
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(ProbeHarnessError::Configuration(
            "the pinned browser executable must be a regular file named by its real path".into(),
        ));
    }
    let directory = executable
        .parent()
        .ok_or_else(|| {
            ProbeHarnessError::Configuration(
                "the pinned browser executable has no bundle directory".into(),
            )
        })?
        .to_path_buf();
    Ok((hash_tree(&directory)?, directory))
}

fn verify_browser_bundle(
    configuration: &ProbeHarnessConfiguration,
) -> Result<VerifiedBrowser, ProbeHarnessError> {
    let executable = configuration.browser_executable()?.to_path_buf();
    let pin = configuration.browser_bundle_pin()?;
    let (observed, directory) = browser_bundle_digest(&executable)?;
    if observed != pin {
        return Err(ProbeHarnessError::HarnessProvenance(format!(
            "the browser bundle does not match this build's configured digest (configured {pin}, \
             observed {observed})"
        )));
    }
    let (product, version) = browser_version(&executable)?;
    Ok(VerifiedBrowser {
        executable,
        directory,
        bundle_sha256: observed,
        product,
        version,
    })
}

struct VerifiedBrowser {
    executable: PathBuf,
    directory: PathBuf,
    bundle_sha256: String,
    /// The whole `--version` line, e.g. `Google Chrome for Testing 151.0.7922.34`.
    product: String,
    /// Its dotted version, e.g. `151.0.7922.34`.
    version: String,
}

/// Asks the pinned bytes for their version. As trustworthy as the bytes, which
/// were hashed against the pin first; `Browser.getVersion` and the startup
/// frame's user agent must carry the same dotted version or the launch refuses.
fn browser_version(executable: &Path) -> Result<(String, String), ProbeHarnessError> {
    let output = Command::new(executable)
        .arg("--version")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .map_err(|error| {
            ProbeHarnessError::NodeProvenance(format!(
                "could not ask the pinned browser executable for its version: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(ProbeHarnessError::NodeProvenance(
            "the pinned browser executable did not report a version".into(),
        ));
    }
    let product = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let version = product
        .split_whitespace()
        .find(|token| {
            token.split('.').count() == 4
                && token
                    .split('.')
                    .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        })
        .map(str::to_owned)
        .ok_or_else(|| {
            ProbeHarnessError::NodeProvenance(format!(
                "the pinned browser executable reported no dotted version in {product:?}"
            ))
        })?;
    if product.is_empty() || product.len() > 256 {
        return Err(ProbeHarnessError::NodeProvenance(
            "the pinned browser executable reported an unusable version string".into(),
        ));
    }
    Ok((product, version))
}

/// Rust-authored bytes only. Reads `source-<index>.ts` beside itself, strips
/// each with the pinned Node's own transformer, and prints one digest line per
/// module. This is the browser profile's form of the Node profiles' in-worker
/// transformer check: the browser cannot strip types, so pinned Node has to
/// reproduce every derived module *before* it is served.
const STRIP_REPRODUCER: &str = r#"import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { stripTypeScriptTypes } from "node:module";
const count = Number(process.argv[2]);
const lines = [];
for (let index = 0; index < count; index += 1) {
  const source = readFileSync(new URL(`./source-${index}.ts`, import.meta.url), "utf8");
  const output = stripTypeScriptTypes(source, { mode: "strip" });
  lines.push(`sha256:${createHash("sha256").update(output).digest("hex")}`);
}
process.stdout.write(lines.join("\n") + "\n");
"#;

/// Refuses unless pinned Node's strip-only output equals the native expected
/// bytes for every module of the graph.
fn verify_node_reproduces_derived_bytes(
    node_executable: &Path,
    module: &InertModule,
) -> Result<(), ProbeHarnessError> {
    let directory = create_private_directory("browser-transform")?;
    let result = (|| {
        let modules = graph_modules(module);
        for (index, derived) in modules.iter().enumerate() {
            write_private_file(
                &directory.join(format!("source-{index}.ts")),
                derived.source,
            )?;
        }
        let script = directory.join("reproduce.mjs");
        write_private_file(&script, STRIP_REPRODUCER.as_bytes())?;
        write_private_file(
            &directory.join("package.json"),
            super::PRIVATE_PACKAGE_SCOPE_MANIFEST,
        )?;
        let output = Command::new(node_executable)
            .arg(&script)
            .arg(modules.len().to_string())
            .current_dir(&directory)
            .env_clear()
            .env("HOME", &directory)
            .env("TMPDIR", &directory)
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("NODE_OPTIONS", "")
            .stdin(Stdio::null())
            .output()
            .map_err(|error| {
                ProbeHarnessError::NodeProvenance(format!(
                    "could not ask the pinned Node executable to reproduce the derived modules: \
                     {error}"
                ))
            })?;
        if !output.status.success() {
            return Err(ProbeHarnessError::NodeProvenance(format!(
                "the pinned Node executable could not strip the graph: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let reported = text.lines().collect::<Vec<_>>();
        if reported.len() != modules.len() {
            return Err(ProbeHarnessError::Protocol(
                "the pinned Node executable reported a different number of derived modules".into(),
            ));
        }
        for (derived, observed) in modules.iter().zip(reported) {
            if observed != derived.output_digest {
                return Err(ProbeHarnessError::Protocol(format!(
                    "pinned Node strip-only output for {} ({observed}) differs from the native \
                     expected derived bytes ({})",
                    derived.source_path, derived.output_digest
                )));
            }
        }
        Ok(())
    })();
    let _ = remove_private_tree(&directory);
    result
}

/// One module of the graph as this launcher sees it: the package-relative
/// source path, the authenticated source bytes, the native derived output and
/// both digests.
struct GraphModule<'a> {
    source_path: &'a str,
    source: &'a [u8],
    output: &'a str,
    output_digest: &'a str,
}

fn graph_modules(module: &InertModule) -> Vec<GraphModule<'_>> {
    module
        .modules
        .iter()
        .map(|derived| GraphModule {
            source_path: &derived.source_path,
            source: derived.source.as_bytes(),
            output: &derived.output,
            output_digest: &derived.output_digest,
        })
        .collect()
}

fn module_url(source_path: &str) -> String {
    format!("{SYNTHETIC_ORIGIN}{MODULE_PREFIX}{source_path}")
}

fn recipe_url(file_name: &str) -> String {
    format!("{SYNTHETIC_ORIGIN}{RECIPE_PREFIX}{file_name}")
}

fn document_url() -> String {
    format!("{SYNTHETIC_ORIGIN}{DOCUMENT_PATH}")
}

/// Resolves a relative specifier (`./x`, `../y`) against a module URL the way
/// the browser's URL parser does for the admitted grammar: the importer's last
/// segment is dropped, `.` and `..` segments are applied, nothing else is
/// interpreted. The grammar already refused `?`, `#` and `\`.
fn resolve_relative(importer_url: &str, specifier: &str) -> Option<String> {
    let path = importer_url.strip_prefix(SYNTHETIC_ORIGIN)?;
    let mut segments = path.split('/').collect::<Vec<_>>();
    segments.pop()?;
    for segment in specifier.split('/') {
        match segment {
            "." | "" => {}
            ".." => {
                // The origin root is segments[0] == "" ; `MODULE_PREFIX` keeps
                // every module below `/modules/`, and an escape above it is
                // simply an unmapped URL, which refuses.
                if segments.len() <= 1 {
                    return None;
                }
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    Some(format!("{SYNTHETIC_ORIGIN}{}", segments.join("/")))
}

/// What one launch serves, keyed by exact URL.
struct ServedResponse {
    content_type: &'static str,
    body: Vec<u8>,
    /// Only the document carries a policy.
    csp: Option<String>,
}

struct ServedMap {
    responses: BTreeMap<String, ServedResponse>,
}

fn build_served_map(
    plan: &CertificationPlan,
    module: &InertModule,
    recipe_file_name: &str,
    recipe_bytes: &[u8],
    import_map_nonce: &str,
) -> Result<ServedMap, ProbeHarnessError> {
    let mut responses = BTreeMap::new();
    let root_url = module_url(&module.source_path);
    let mut imports = serde_json::Map::new();
    imports.insert(
        plan.resolved_import.specifier.clone(),
        serde_json::Value::String(root_url),
    );
    let mut scopes = serde_json::Map::new();
    for edge in &module.edges {
        let importer = module_url(&edge.importer_path);
        let computed = resolve_relative(&importer, &edge.specifier).ok_or_else(|| {
            ProbeHarnessError::Configuration(format!(
                "relative edge {:?} from {} cannot be expressed as a URL",
                edge.specifier, edge.importer_path
            ))
        })?;
        let target = module_url(&edge.target_path);
        let scope = scopes
            .entry(importer)
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        let Some(scope) = scope.as_object_mut() else {
            unreachable!("scopes are objects");
        };
        if scope
            .insert(computed, serde_json::Value::String(target))
            .is_some()
        {
            return Err(ProbeHarnessError::Configuration(
                "the authenticated edge map names one specifier twice for one importer".into(),
            ));
        }
    }
    let import_map = serde_json::json!({ "imports": imports, "scopes": scopes });
    let document = format!(
        "<!doctype html><meta charset=\"utf-8\"><script type=\"importmap\" \
         nonce=\"{import_map_nonce}\">{import_map}</script><title>solid-checker probe</title>\n"
    );
    let csp = format!(
        "default-src 'none'; script-src 'nonce-{import_map_nonce}' {SYNTHETIC_ORIGIN}; \
         connect-src 'none'; worker-src 'none'; child-src 'none'; frame-src 'none'; \
         object-src 'none'; base-uri 'none'; form-action 'none'"
    );
    responses.insert(
        document_url(),
        ServedResponse {
            content_type: "text/html; charset=utf-8",
            body: document.into_bytes(),
            csp: Some(csp),
        },
    );
    responses.insert(
        recipe_url(recipe_file_name),
        ServedResponse {
            content_type: "text/javascript; charset=utf-8",
            body: recipe_bytes.to_vec(),
            csp: None,
        },
    );
    for derived in graph_modules(module) {
        if responses
            .insert(
                module_url(derived.source_path),
                ServedResponse {
                    content_type: "text/javascript; charset=utf-8",
                    body: derived.output.as_bytes().to_vec(),
                    csp: None,
                },
            )
            .is_some()
        {
            return Err(ProbeHarnessError::Configuration(
                "two graph modules share one served URL".into(),
            ));
        }
    }
    Ok(ServedMap { responses })
}

/// The execution binding the bootstrap must echo. Paths are the served URLs:
/// there is no private file the browser reads, the bytes are the derived
/// bytes, and the receipt binds their digests.
fn execution_request(module: &InertModule, consumer: bool) -> InertExecutionRequest {
    let root_url = module_url(&module.source_path);
    InertExecutionRequest {
        profile: module.profile.into(),
        source_path: root_url.clone(),
        derived_path: root_url,
        source_digest: module.source_digest.clone(),
        output_digest: module.output_digest.clone(),
        export_name: module.export_name.clone(),
        consumer,
        modules: module
            .modules
            .iter()
            .map(|derived| DerivedExecutionModuleRequest {
                source_path: module_url(&derived.source_path),
                derived_path: module_url(&derived.source_path),
                source_digest: derived.source_digest.clone(),
                output_digest: derived.output_digest.clone(),
            })
            .collect(),
        edges: module
            .edges
            .iter()
            .map(|edge| DerivedExecutionEdgeRequest {
                importer_path: module_url(&edge.importer_path),
                specifier: edge.specifier.clone(),
                target_path: module_url(&edge.target_path),
            })
            .collect(),
    }
}

/// The page bootstrap. Rust-authored bytes compiled into the verifier and
/// instantiated per launch; every `__NAME__` token is replaced by a JSON
/// literal before injection, so nothing here is read from the page.
///
/// Every primordial the report path needs is captured while this runs, which
/// is before the document — and therefore before the import map, the recipe,
/// and the package — exists. The frame representation and serializer are the
/// Node harness's, ported: null-prototype records, own keys by index, no
/// `toJSON`, scalars only through the captured `JSON.stringify`.
const BOOTSTRAP: &str = r#"(() => {
  "use strict";
  const report = window.__BINDING__;
  delete window.__BINDING__;
  if (typeof report !== "function") return;
  const NONCE = __NONCE__;
  const PID = __PID__;
  const SESSION = __SESSION__;
  const EXECUTION = __EXECUTION__;
  const RECIPE_URL = __RECIPE_URL__;
  const CONSUMER = __CONSUMER__;
  const create = Object.create, ownKeys = Object.keys, freeze = Object.freeze, protoOf = Object.getPrototypeOf,
    hasOwn = Object.hasOwn, isArray = Array.isArray, isFinite = Number.isFinite, apply = Reflect.apply,
    encodeScalar = JSON.stringify, TypeErrorC = TypeError, ErrorC = Error, PromiseC = Promise,
    resolvePromise = Promise.resolve.bind(Promise), scheduleMacrotask = window.setTimeout.bind(window),
    requestFrame = window.requestAnimationFrame.bind(window), addListener = window.addEventListener.bind(window),
    randomUUID = crypto.randomUUID.bind(crypto), subtleDigest = crypto.subtle.digest.bind(crypto.subtle),
    TextEncoderC = TextEncoder, Uint8ArrayC = Uint8Array, asString = String,
    readyState = document.readyState, userAgent = navigator.userAgent, importModule = (url) => import(url);
  const HEX = [];
  for (let index = 0; index < 256; index += 1) HEX[index] = (index < 16 ? "0" : "") + index.toString(16);
  freeze(Object.prototype); freeze(Array.prototype); freeze(Function.prototype);
  const FRAME_LIST = Symbol("solid-checker-probe-frame-list");
  const record = () => create(null);
  const list = () => { const value = create(null); value[FRAME_LIST] = true; value.length = 0; return value; };
  const append = (target, value) => { target[target.length] = value; target.length += 1; return target; };
  const adopt = (value, depth = 0) => {
    if (value === null) return null;
    const kind = typeof value;
    if (kind === "string" || kind === "boolean") return value;
    if (kind === "number") { if (!isFinite(value)) throw new TypeErrorC("frame number"); return value; }
    if (kind !== "object" || depth >= 32) throw new TypeErrorC("frame value");
    if (isArray(value)) { const out = list(); for (let i = 0; i < value.length; i += 1) append(out, adopt(value[i], depth + 1)); return out; }
    const out = record(); const keys = ownKeys(value);
    for (let i = 0; i < keys.length; i += 1) out[keys[i]] = adopt(value[keys[i]], depth + 1);
    return out;
  };
  const serialize = (value) => {
    if (value === null) return "null";
    const kind = typeof value;
    if (kind === "string" || kind === "boolean") return encodeScalar(value);
    if (kind === "number") { if (!isFinite(value)) throw new TypeErrorC("frame number"); return encodeScalar(value); }
    if (kind !== "object") throw new TypeErrorC("frame value");
    if (protoOf(value) !== null) throw new TypeErrorC("frame prototype");
    if (value[FRAME_LIST] === true) { let text = "["; for (let i = 0; i < value.length; i += 1) { if (i > 0) text += ","; text += serialize(value[i]); } return text + "]"; }
    let text = "{"; const keys = ownKeys(value);
    for (let i = 0; i < keys.length; i += 1) { if (i > 0) text += ","; text += encodeScalar(keys[i]) + ":" + serialize(value[keys[i]]); }
    return text + "}";
  };
  const digestText = async (text) => {
    const bytes = new TextEncoderC().encode(text);
    const hash = new Uint8ArrayC(await subtleDigest("SHA-256", bytes));
    let hex = ""; for (let i = 0; i < hash.length; i += 1) hex += HEX[hash[i]];
    return "sha256:" + hex;
  };
  const startup = record();
  startup.format = "solid-checker-probe-browser-startup";
  startup.protocol = "solid-checker-runtime-probe-browser-v1";
  startup.nonce = NONCE;
  startup.userAgent = userAgent;
  startup.readyState = readyState;
  report(serialize(startup));
  const sessionId = adopt(SESSION.id);
  const environment = adopt(SESSION.mode.environment);
  const execution = record();
  execution.binding = adopt(EXECUTION);
  execution.loaded = false;
  execution.consumerCompleted = false;
  execution.stage = "requested";
  const isolation = record();
  isolation.process = PID + ":" + randomUUID();
  isolation.realm = randomUUID();
  isolation.moduleInstance = randomUUID();
  const recorded = list();
  let microtasks = 0, macrotasks = 0, animationFrames = 0;
  const copyEvent = (event, sequence) => {
    if (!event || typeof event !== "object") throw new TypeErrorC("runtime probe events must be objects");
    const copy = record(); const keys = ownKeys(event);
    for (let i = 0; i < keys.length; i += 1) {
      const key = keys[i], value = event[key], kind = typeof value;
      if (kind === "string" || kind === "boolean" || (kind === "number" && isFinite(value))) { copy[key] = value; continue; }
      throw new TypeErrorC("runtime probe event field " + key + " must be a string, a finite number, or a boolean");
    }
    if (typeof copy.marker !== "string") throw new TypeErrorC("runtime probe events require a marker");
    copy.sequence = sequence;
    return freeze(copy);
  };
  const harness = freeze({
    emit(event) { append(recorded, copyEvent(event, recorded.length)); },
    async drain(controls) {
      const steps = SESSION.drain;
      if (!isArray(steps)) throw new TypeErrorC("a probe session's drain plan must be a list of steps");
      for (let position = 0; position < steps.length; position += 1) {
        const step = steps[position];
        if (step === null || typeof step !== "object") throw new TypeErrorC("a probe session's drain step must be a record");
        const kind = hasOwn(step, "kind") ? step.kind : undefined;
        const maxTurns = hasOwn(step, "maxTurns") ? step.maxTurns : undefined;
        if (kind === "flush") {
          const flush = controls !== null && controls !== undefined && hasOwn(controls, "flush") ? controls.flush : undefined;
          if (typeof flush !== "function") throw new TypeErrorC("runtime probe recipe did not provide the planned flush control");
          await apply(flush, controls, []);
        } else if (kind === "microtasks") {
          for (let turn = 0; turn < maxTurns; turn += 1) { await resolvePromise(); microtasks += 1; }
        } else if (kind === "macrotasks") {
          for (let turn = 0; turn < maxTurns; turn += 1) { await new PromiseC((resolve) => scheduleMacrotask(resolve, 0)); macrotasks += 1; }
        } else if (kind === "animation-frames") {
          for (let turn = 0; turn < maxTurns; turn += 1) { await new PromiseC((resolve) => requestFrame(() => resolve())); animationFrames += 1; }
        } else {
          throw new TypeErrorC("unknown runtime probe drain step " + kind);
        }
      }
    },
    events() { const copy = list(); for (let i = 0; i < recorded.length; i += 1) append(copy, recorded[i]); return copy; }
  });
  const run = async () => {
    let outcome = record();
    try {
      const module = await importModule(RECIPE_URL);
      execution.loaded = true;
      execution.stage = "module-loaded";
      if (typeof module.runProbeSession !== "function") {
        outcome.kind = "refused";
        outcome.reason = "recipe module exports no runProbeSession function";
      } else {
        const controls = await module.runProbeSession(SESSION, harness);
        await harness.drain(controls);
        outcome.kind = "completed";
        outcome.events = harness.events();
        if (CONSUMER) { execution.consumerCompleted = true; execution.stage = "consumer-completed"; }
      }
    } catch (error) {
      outcome = record();
      outcome.kind = "error";
      outcome.details = await digestText(error instanceof ErrorC ? (error.stack ?? error.message) : asString(error));
    }
    const frame = record();
    frame.session = sessionId;
    frame.environment = environment;
    frame.isolation = isolation;
    frame.execution = execution;
    frame.drainedMicrotasks = microtasks;
    frame.drainedMacrotasks = macrotasks;
    frame.drainedAnimationFrames = animationFrames;
    frame.outcome = outcome;
    report(serialize(frame));
  };
  addListener("DOMContentLoaded", () => { run().catch(() => {}); }, { once: true });
})();
"#;

fn instantiate_bootstrap(
    nonce: &str,
    process_id: u32,
    session_bytes: &[u8],
    execution: &InertExecutionRequest,
    recipe_url: &str,
    consumer: bool,
) -> Result<String, ProbeHarnessError> {
    let json = |value: &serde_json::Value| {
        serde_json::to_string(value).map_err(|error| ProbeHarnessError::Protocol(error.to_string()))
    };
    let session: serde_json::Value = serde_json::from_slice(session_bytes)
        .map_err(|error| ProbeHarnessError::Protocol(error.to_string()))?;
    let execution = serde_json::to_value(execution)
        .map_err(|error| ProbeHarnessError::Protocol(error.to_string()))?;
    Ok(BOOTSTRAP
        .replace("__BINDING__", REPORT_BINDING)
        .replace(
            "__NONCE__",
            &json(&serde_json::Value::String(nonce.into()))?,
        )
        .replace(
            "__PID__",
            &json(&serde_json::Value::String(process_id.to_string()))?,
        )
        .replace("__SESSION__", &json(&session)?)
        .replace("__EXECUTION__", &json(&execution)?)
        .replace(
            "__RECIPE_URL__",
            &json(&serde_json::Value::String(recipe_url.into()))?,
        )
        .replace("__CONSUMER__", if consumer { "true" } else { "false" }))
}

/// The private workspace of one browser transaction: the carve-out directory
/// and the watched census around it.
struct BrowserWorkspace {
    directory: PathBuf,
    profile_directory: PathBuf,
    watched: Vec<(String, WatchedInput)>,
    pinned: Vec<(String, String)>,
    before: BTreeMap<String, String>,
}

impl BrowserWorkspace {
    fn create(
        browser: &VerifiedBrowser,
        node_executable: &Path,
        node_executable_sha256: &str,
        type_facts_pin: &TypeFactsProducerPin,
    ) -> Result<Self, ProbeHarnessError> {
        let directory = create_private_directory("browser")?;
        let profile_directory = directory.join(PROFILE_DIRECTORY);
        fs::create_dir(&profile_directory)?;
        set_directory_permissions(&profile_directory)?;
        let mut watched = vec![
            (
                "private-directory-entries".to_owned(),
                WatchedInput::DirectoryEntries(directory.clone()),
            ),
            (
                "browser-bundle".to_owned(),
                WatchedInput::Contents(browser.directory.clone()),
            ),
            (
                "node-executable".to_owned(),
                WatchedInput::Contents(node_executable.to_path_buf()),
            ),
            (
                "type-facts-image".to_owned(),
                WatchedInput::Contents(type_facts_pin.path().to_path_buf()),
            ),
        ];
        if let Ok(verifier) = std::env::current_exe() {
            watched.push((
                "verifier-image".to_owned(),
                WatchedInput::Contents(verifier),
            ));
        }
        let mut workspace = Self {
            directory,
            profile_directory,
            watched,
            pinned: vec![
                ("browser-bundle".to_owned(), browser.bundle_sha256.clone()),
                (
                    "node-executable".to_owned(),
                    node_executable_sha256.to_owned(),
                ),
            ],
            before: BTreeMap::new(),
        };
        workspace.before = workspace.watch_digests()?;
        workspace.verify_pinned(&workspace.before)?;
        Ok(workspace)
    }

    fn watch_digests(&self) -> Result<BTreeMap<String, String>, ProbeHarnessError> {
        let mut census = BTreeMap::new();
        for (label, input) in &self.watched {
            let digest = watch_digest(input)?;
            if digest == ABSENT_WATCHED_PATH
                && matches!(label.as_str(), "browser-bundle" | "node-executable")
            {
                return Err(ProbeHarnessError::IsolationViolation(format!(
                    "watched probe input {label} is absent"
                )));
            }
            if census.insert(label.clone(), digest).is_some() {
                return Err(ProbeHarnessError::IsolationViolation(format!(
                    "the watched probe input census names {label} twice"
                )));
            }
        }
        Ok(census)
    }

    fn verify_pinned(&self, census: &BTreeMap<String, String>) -> Result<(), ProbeHarnessError> {
        for (label, expected) in &self.pinned {
            match census.get(label).map(String::as_str) {
                Some(observed) if observed == expected => {}
                Some(observed) => {
                    return Err(ProbeHarnessError::IsolationViolation(format!(
                        "watched probe input {label} is {observed}, not the {expected} this build \
                         pinned"
                    )));
                }
                None => {
                    return Err(ProbeHarnessError::IsolationViolation(format!(
                        "the watched probe input census no longer covers {label}"
                    )));
                }
            }
        }
        Ok(())
    }

    fn verify_unchanged(&self) -> Result<(), ProbeHarnessError> {
        let after = self.watch_digests()?;
        self.verify_pinned(&after)?;
        for (label, before) in &self.before {
            match after.get(label) {
                Some(after) if after == before => {}
                Some(_) => {
                    return Err(ProbeHarnessError::IsolationViolation(format!(
                        "{label} changed while the browser probe was executing"
                    )));
                }
                None => {
                    return Err(ProbeHarnessError::IsolationViolation(format!(
                        "the watched probe input census no longer covers {label}"
                    )));
                }
            }
        }
        if after.len() != self.before.len() {
            return Err(ProbeHarnessError::IsolationViolation(
                "the watched probe input census changed during the run".into(),
            ));
        }
        Ok(())
    }
}

impl Drop for BrowserWorkspace {
    fn drop(&mut self) {
        let _ = remove_private_tree(&self.directory);
    }
}

/// Executes every mandatory gate of `schedule` in the pinned headless shell.
pub(crate) fn run_browser_gates(
    plan: &CertificationPlan,
    schedule: &ProbeGateSchedule,
    configuration: &ProbeHarnessConfiguration,
    type_facts_pin: &TypeFactsProducerPin,
    module: &InertModule,
    consumer: bool,
) -> Result<(RuntimeProbeEvaluation, BoundProbeHarnessIdentity), ProbeHarnessError> {
    if schedule.gates().is_empty() {
        return Err(ProbeHarnessError::Configuration(
            "an empty probe schedule needs no harness launch".into(),
        ));
    }
    if module.modules.is_empty() {
        return Err(ProbeHarnessError::Configuration(
            "the browser profile requires an explicit derived module graph".into(),
        ));
    }
    let pin = configuration.pin()?;
    let node_executable = configuration.node_executable.as_path();
    let node = verify_node_executable(node_executable, &pin)?;
    let image = verify_harness_image(&configuration.harness_root, &pin)?;
    let node_version = node_version(node_executable, &node)?;
    let browser = verify_browser_bundle(configuration)?;
    verify_node_reproduces_derived_bytes(node_executable, module)?;

    let corpus = RecipeCorpus::load(configuration.recipe_corpus(), plan)?;
    if !plan.verified_closure.manifest().dependencies.is_empty() {
        return Err(ProbeHarnessError::Configuration(
            "the browser profile requires an empty runtime dependency closure".into(),
        ));
    }
    let requested = requested_conditions(plan)?;
    let environment = browser_environment(&browser, &requested);
    let runtime_plan = plan.runtime_probe_plan(schedule, &corpus, environment)?;
    for gate in schedule.gates() {
        let recipe = corpus.recipe_for(gate.semantic_claim_id()).ok_or_else(|| {
            ProbeHarnessError::MissingRecipe {
                gate_id: gate.id().to_owned(),
                semantic_claim_id: gate.semantic_claim_id().to_owned(),
            }
        })?;
        if recipe.import_kind() != super::ProbeImportKind::Esm {
            return Err(ProbeHarnessError::Configuration(
                "browser execution requires ESM recipes".into(),
            ));
        }
        if !recipe.dependency_specifiers().is_empty() {
            return Err(ProbeHarnessError::Configuration(
                "browser execution admits no dependency specifiers".into(),
            ));
        }
    }
    let empty_dependencies = BTreeMap::new();
    let workspace = BrowserWorkspace::create(&browser, node_executable, &node, type_facts_pin)?;

    let launched = (|| {
        let mut runs = Vec::with_capacity(runtime_plan.sessions().len());
        for session in runtime_plan.sessions() {
            let claim_id = session.claim_id().as_str();
            let recipe = corpus.recipe_for(claim_id).ok_or_else(|| {
                ProbeHarnessError::RecipeProvenance(format!(
                    "the recipe corpus stopped naming claim {claim_id} mid-transaction"
                ))
            })?;
            let session_bytes = runtime_probe_wire::encode_probe_session(
                session,
                &format!("recipes/{}", recipe.file_name),
                recipe.construction(),
                None,
            )?;
            let run = launch_browser_session(
                &workspace,
                &browser,
                plan,
                module,
                &recipe.file_name,
                &recipe.bytes,
                &session_bytes,
                session.policy().timeout_millis,
                consumer,
            )?;
            workspace.verify_unchanged()?;
            runs.push(run);
        }
        Ok::<_, ProbeHarnessError>(runs)
    })();
    let runs = match (launched, workspace.verify_unchanged()) {
        (_, Err(violation)) => return Err(violation),
        (Err(error), Ok(())) => return Err(error),
        (Ok(runs), Ok(())) => runs,
    };

    let producer = ToolIdentity {
        name: "solid-checker-probe-browser-harness".into(),
        version: browser.version.clone(),
        build: Digest::parse(browser.bundle_sha256.clone())
            .expect("the bundle digest is canonical"),
        protocol: Some(BROWSER_PROTOCOL.into()),
    };
    let evaluation = evaluate_runtime_probes(&runtime_plan, runs, producer)?;
    let identity = BoundProbeHarnessIdentity {
        execution: HarnessExecution::ControlledInert,
        snapshot_root: plan.snapshot.root().to_owned(),
        demand_graph_root: plan.demand_graph.root().as_str().to_owned(),
        gate_ids: schedule
            .gates()
            .iter()
            .map(|gate| gate.id().to_owned())
            .collect(),
        fields: vec![
            format!("harness-manifest:{}", image.manifest_sha256),
            format!("node-executable:{}", pin.node_executable_sha256),
            format!("node-version:{node_version}"),
            format!("browser-bundle:{}", browser.bundle_sha256),
            format!("browser-version:{}", browser.version),
            format!("browser-product:{}", browser.product),
            format!("browser-protocol:{BROWSER_PROTOCOL}"),
            format!(
                "sandbox-policy:{}",
                browser_sandbox_policy_digest().as_str()
            ),
            format!("runtime-probe-plan:{}", runtime_plan.digest().as_str()),
            format!("recipe-corpus:{}", corpus.root.as_str()),
            format!(
                "dependency-materialization:{}",
                dependency_materialization_root(&empty_dependencies)
            ),
        ],
    };
    let mut identity = identity;
    identity.fields.extend(module.binding());
    Ok((evaluation, identity))
}

fn browser_environment(browser: &VerifiedBrowser, requested: &[String]) -> EnvironmentIdentity {
    let mut conditions = requested
        .iter()
        .map(|condition| format!("requested:{condition}"))
        .collect::<Vec<_>>();
    conditions.push("resolver:checker-exact-url-map".into());
    EnvironmentIdentity {
        runtime: ToolIdentity {
            name: "chromium-headless-shell".into(),
            version: browser.version.clone(),
            build: Digest::parse(browser.bundle_sha256.clone())
                .expect("the bundle digest is canonical"),
            protocol: Some(BROWSER_PROTOCOL.into()),
        },
        os: std::env::consts::OS.into(),
        architecture: std::env::consts::ARCH.into(),
        conditions,
        sandbox: SandboxIdentity {
            kind: SandboxKind::Process,
            policy: Some(browser_sandbox_policy_digest()),
        },
    }
}

/// The flags one launch is built from. Nothing from the environment.
fn browser_arguments(profile_directory: &Path) -> Vec<String> {
    vec![
        "--headless".into(),
        "--remote-debugging-pipe".into(),
        format!("--user-data-dir={}", profile_directory.display()),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "--disable-background-networking".into(),
        "--disable-component-update".into(),
        "--disable-sync".into(),
        "--disable-crash-reporter".into(),
        "--disable-breakpad".into(),
        "--disable-gpu".into(),
        "--disable-gpu-shader-disk-cache".into(),
        "--disable-extensions".into(),
        "--disable-features=ServiceWorker,Translate,OptimizationHints,MediaRouter".into(),
        "--host-resolver-rules=MAP * ~NOTFOUND".into(),
        "--deny-permission-prompts".into(),
        "--enable-automation".into(),
        "about:blank".into(),
    ]
}

#[allow(clippy::too_many_arguments)]
fn launch_browser_session(
    workspace: &BrowserWorkspace,
    browser: &VerifiedBrowser,
    plan: &CertificationPlan,
    module: &InertModule,
    recipe_file_name: &str,
    recipe_bytes: &[u8],
    session_bytes: &[u8],
    timeout_millis: u64,
    consumer: bool,
) -> Result<ProbeRun, ProbeHarnessError> {
    let epoch_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let nonce = format!(
        "{:x}-{:x}-{:x}",
        std::process::id(),
        epoch_nanos,
        PRIVATE_HARNESS_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let import_map_nonce = format!(
        "{:x}",
        Sha256::digest(format!("import-map:{nonce}").as_bytes())
    );
    let served = build_served_map(
        plan,
        module,
        recipe_file_name,
        recipe_bytes,
        &import_map_nonce[..32],
    )?;
    let execution = execution_request(module, consumer);

    let mut command = Command::new(&browser.executable);
    command
        .args(browser_arguments(&workspace.profile_directory))
        .current_dir(&workspace.directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    command.env_clear();
    command.env("HOME", &workspace.directory);
    command.env("TMPDIR", &workspace.directory);
    command.env("LANG", "C");
    command.env("LC_ALL", "C");

    let (child, to_browser, from_browser) = spawn_piped_browser(&mut command)?;
    let mut worker = WorkerProcess::new(child);
    let process_id = worker.process_id();
    let diagnostics = spawn_stderr_reader(worker.take_stderr());
    let deadline = Instant::now() + Duration::from_millis(timeout_millis.max(1));
    let startup_deadline =
        Instant::now() + STARTUP_BUDGET.min(deadline.saturating_duration_since(Instant::now()));
    let mut client = CdpClient::new(to_browser, from_browser);

    let outcome = drive_session(
        &mut client,
        browser,
        &served,
        &execution,
        &nonce,
        process_id,
        session_bytes,
        recipe_file_name,
        consumer,
        startup_deadline,
        deadline,
    );
    // Nothing more is wanted from the browser; its whole group dies here.
    drop(worker);
    match outcome {
        Ok(run) => Ok(run),
        Err(error) => {
            let tail = diagnostics
                .recv_timeout(EXTRA_FRAME_GRACE)
                .unwrap_or_default();
            Err(match error {
                ProbeHarnessError::Launch(message) if !tail.is_empty() => {
                    ProbeHarnessError::Launch(format!("{message} (browser stderr: {tail})"))
                }
                other => other,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn drive_session(
    client: &mut CdpClient,
    browser: &VerifiedBrowser,
    served: &ServedMap,
    execution: &InertExecutionRequest,
    nonce: &str,
    process_id: u32,
    session_bytes: &[u8],
    recipe_file_name: &str,
    consumer: bool,
    startup_deadline: Instant,
    deadline: Instant,
) -> Result<ProbeRun, ProbeHarnessError> {
    let version = client.call(
        "Browser.getVersion",
        serde_json::json!({}),
        None,
        startup_deadline,
    )?;
    let product = version["product"].as_str().unwrap_or_default();
    if !product.contains(&browser.version) {
        return Err(ProbeHarnessError::Protocol(format!(
            "the browser reports product {product:?}; the pinned executable reported version {:?}",
            browser.version
        )));
    }
    client.call(
        "Browser.setDownloadBehavior",
        serde_json::json!({ "behavior": "deny" }),
        None,
        startup_deadline,
    )?;
    let target = client.call(
        "Target.createTarget",
        serde_json::json!({ "url": "about:blank" }),
        None,
        startup_deadline,
    )?;
    let target_id = target["targetId"]
        .as_str()
        .ok_or_else(|| ProbeHarnessError::Protocol("the browser created no target".into()))?
        .to_owned();
    let attached = client.call(
        "Target.attachToTarget",
        serde_json::json!({ "targetId": target_id, "flatten": true }),
        None,
        startup_deadline,
    )?;
    let session_id = attached["sessionId"]
        .as_str()
        .ok_or_else(|| ProbeHarnessError::Protocol("the browser attached no session".into()))?
        .to_owned();
    let page = Some(session_id.as_str());
    client.call("Page.enable", serde_json::json!({}), page, startup_deadline)?;
    client.call(
        "Runtime.enable",
        serde_json::json!({}),
        page,
        startup_deadline,
    )?;
    client.call(
        "Target.setAutoAttach",
        serde_json::json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true }),
        page,
        startup_deadline,
    )?;
    client.call(
        "Fetch.enable",
        serde_json::json!({ "patterns": [{ "urlPattern": "*", "requestStage": "Request" }] }),
        page,
        startup_deadline,
    )?;
    client.call(
        "Runtime.addBinding",
        serde_json::json!({ "name": REPORT_BINDING }),
        page,
        startup_deadline,
    )?;
    let bootstrap = instantiate_bootstrap(
        nonce,
        process_id,
        session_bytes,
        execution,
        &recipe_url(recipe_file_name),
        consumer,
    )?;
    client.call(
        "Page.addScriptToEvaluateOnNewDocument",
        serde_json::json!({ "source": bootstrap }),
        page,
        startup_deadline,
    )?;
    // Not awaited: the navigation response arrives only after the navigation
    // commits, and committing needs the paused document request answered by
    // the event loop below. Its eventual response carries no method and is
    // dropped by `next_event`.
    client.notify(
        "Page.navigate",
        serde_json::json!({ "url": document_url() }),
        page,
    )?;

    let mut requested = BTreeMap::<String, u32>::new();
    let mut main_context: Option<i64> = None;
    let mut startup_seen = false;
    let mut run_frame: Option<String> = None;
    let mut frames = 0_usize;
    while run_frame.is_none() {
        let event = client.next_event(deadline)?;
        let method = event["method"].as_str().unwrap_or_default();
        let event_session = event["sessionId"].as_str();
        match method {
            "Fetch.requestPaused" => {
                let params = &event["params"];
                let request_id = params["requestId"].as_str().unwrap_or_default().to_owned();
                let url = params["request"]["url"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                *requested.entry(url.clone()).or_insert(0) += 1;
                match served.responses.get(&url) {
                    Some(response) => {
                        let mut headers = vec![
                            serde_json::json!({ "name": "Content-Type", "value": response.content_type }),
                            serde_json::json!({ "name": "Cache-Control", "value": "no-store" }),
                        ];
                        if let Some(csp) = &response.csp {
                            headers.push(serde_json::json!({ "name": "Content-Security-Policy", "value": csp }));
                        }
                        client.call(
                            "Fetch.fulfillRequest",
                            serde_json::json!({
                                "requestId": request_id,
                                "responseCode": 200,
                                "responseHeaders": headers,
                                "body": STANDARD.encode(&response.body),
                            }),
                            event_session,
                            deadline,
                        )?;
                    }
                    None => {
                        let _ = client.call(
                            "Fetch.failRequest",
                            serde_json::json!({ "requestId": request_id, "errorReason": "BlockedByClient" }),
                            event_session,
                            deadline,
                        );
                        return Err(ProbeHarnessError::ConditionMismatch(format!(
                            "the page requested {url:?}, which is not in the served exact URL map: \
                             the browser would have loaded bytes this transaction never authenticated"
                        )));
                    }
                }
            }
            "Runtime.executionContextCreated" => {
                // The main frame's default (main-world) context, as it stands
                // *now*: navigation replaces the initial about:blank context, and
                // an iframe's default context names another frame. Frames are
                // accepted only from the current one.
                let context = &event["params"]["context"];
                let aux = &context["auxData"];
                if aux["isDefault"].as_bool() == Some(true)
                    && aux["frameId"].as_str() == Some(target_id.as_str())
                {
                    main_context = context["id"].as_i64();
                }
            }
            "Runtime.executionContextDestroyed" => {
                if event["params"]["executionContextId"].as_i64() == main_context {
                    main_context = None;
                }
            }
            "Runtime.bindingCalled" => {
                let params = &event["params"];
                if params["name"].as_str() != Some(REPORT_BINDING) {
                    return Err(ProbeHarnessError::Protocol(
                        "the page called a binding this launcher did not add".into(),
                    ));
                }
                if main_context.is_none() || params["executionContextId"].as_i64() != main_context {
                    return Err(ProbeHarnessError::Protocol(
                        "a probe frame arrived from an execution context other than the page's \
                         main world"
                            .into(),
                    ));
                }
                let payload = params["payload"].as_str().unwrap_or_default();
                if payload.len() > MAX_REPORT_BYTES {
                    return Err(ProbeHarnessError::Protocol(
                        "a probe frame exceeded the report byte budget".into(),
                    ));
                }
                frames += 1;
                if frames == 1 {
                    verify_browser_startup_frame(payload, nonce, &browser.version)?;
                    startup_seen = true;
                } else if frames == 2 {
                    run_frame = Some(payload.to_owned());
                } else {
                    return Err(ProbeHarnessError::Protocol(
                        "the page wrote more than one run frame; exactly one is the protocol"
                            .into(),
                    ));
                }
            }
            "Target.attachedToTarget" => {
                // The attach this launcher performed itself echoes as an event
                // on the browser session; anything else — a worker, an iframe,
                // a popup — is a realm outside the bootstrap and refuses.
                if event["params"]["targetInfo"]["targetId"].as_str() == Some(target_id.as_str())
                    && event_session.is_none()
                {
                    continue;
                }
                return Err(ProbeHarnessError::ConditionMismatch(format!(
                    "the page created a {} target, which the browser profile refuses",
                    event["params"]["targetInfo"]["type"]
                        .as_str()
                        .unwrap_or("new")
                )));
            }
            "Inspector.targetCrashed" | "Target.targetCrashed" => {
                return Err(ProbeHarnessError::Launch("the browser page crashed".into()));
            }
            // An exception the bootstrap did not contain is a launch failure with
            // a name, not a silent timeout: the recipe's own errors are caught
            // and reported as the run outcome, so anything reaching here is the
            // harness's, the import map's or the module graph's.
            "Runtime.exceptionThrown" => {
                let details = &event["params"]["exceptionDetails"];
                let description = details["exception"]["description"]
                    .as_str()
                    .or_else(|| details["text"].as_str())
                    .unwrap_or("uncaught exception");
                return Err(ProbeHarnessError::Launch(format!(
                    "the browser page threw outside the recipe: {}",
                    description.chars().take(512).collect::<String>()
                )));
            }
            _ => {}
        }
    }
    if !startup_seen {
        return Err(ProbeHarnessError::Protocol(
            "no startup frame arrived".into(),
        ));
    }
    // Any frame the page had already written after its run frame is a protocol
    // refusal; give the reader the same grace the Node path gives.
    let grace = Instant::now() + EXTRA_FRAME_GRACE;
    while let Ok(event) = client.next_event(grace) {
        if event["method"].as_str() == Some("Runtime.bindingCalled") {
            return Err(ProbeHarnessError::Protocol(
                "the page wrote more than one run frame; exactly one is the protocol".into(),
            ));
        }
        if event["method"].as_str() == Some("Fetch.requestPaused") {
            let url = event["params"]["request"]["url"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            return Err(ProbeHarnessError::ConditionMismatch(format!(
                "the page requested {url:?} after its run frame"
            )));
        }
    }
    let expected = served.responses.keys().cloned().collect::<BTreeSet<_>>();
    let observed = requested.keys().cloned().collect::<BTreeSet<_>>();
    if expected != observed || requested.values().any(|count| *count != 1) {
        return Err(ProbeHarnessError::ConditionMismatch(format!(
            "the page requested {observed:?} (with counts {requested:?}); the served exact URL \
             map is {expected:?}: the executed graph is not the bound graph"
        )));
    }
    let payload = run_frame.expect("the loop exits only with a run frame");
    let decoded = runtime_probe_wire::decode_probe_run(payload.as_bytes())?;
    if !decoded
        .run
        .isolation
        .process
        .starts_with(&format!("{process_id}:"))
    {
        return Err(ProbeHarnessError::Protocol(format!(
            "the page reported process identity {:?}, which does not name the launched browser \
             {process_id}",
            decoded.run.isolation.process
        )));
    }
    match &decoded.execution {
        Some(actual)
            if actual.binding == *execution
                && actual.loaded
                && actual.consumer_completed == execution.consumer => {}
        _ => {
            return Err(ProbeHarnessError::Protocol(format!(
                "execution profile/input/output/consumer echo mismatch or the root module was not \
                 loaded (stage: {})",
                decoded
                    .execution
                    .as_ref()
                    .map_or("missing", |actual| actual.stage.as_str())
            )));
        }
    }
    if decoded.resolution.is_some() {
        return Err(ProbeHarnessError::Protocol(
            "the browser page reported a resolution echo it has no resolver for".into(),
        ));
    }
    Ok(decoded.run)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireBrowserStartup {
    format: String,
    protocol: String,
    nonce: String,
    user_agent: String,
    ready_state: String,
}

fn verify_browser_startup_frame(
    payload: &str,
    nonce: &str,
    version: &str,
) -> Result<(), ProbeHarnessError> {
    let frame: WireBrowserStartup = serde_json::from_str(payload).map_err(|error| {
        ProbeHarnessError::Protocol(format!("invalid browser startup frame: {error}"))
    })?;
    if frame.format != BROWSER_STARTUP_FORMAT {
        return Err(ProbeHarnessError::Protocol(format!(
            "browser startup frame must use format {BROWSER_STARTUP_FORMAT:?}"
        )));
    }
    if frame.protocol != BROWSER_PROTOCOL {
        return Err(ProbeHarnessError::Protocol(format!(
            "browser bootstrap speaks protocol {:?}; this verifier speaks {BROWSER_PROTOCOL:?}",
            frame.protocol
        )));
    }
    if frame.nonce != nonce {
        return Err(ProbeHarnessError::Protocol(
            "browser bootstrap did not echo this launch's nonce".into(),
        ));
    }
    if frame.ready_state != "loading" {
        return Err(ProbeHarnessError::Protocol(format!(
            "the bootstrap ran at readyState {:?}, not before the document",
            frame.ready_state
        )));
    }
    if !frame.user_agent.contains(version) {
        return Err(ProbeHarnessError::Protocol(format!(
            "the page's user agent {:?} does not carry the pinned browser version {version:?}",
            frame.user_agent
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The CDP pipe client
// ---------------------------------------------------------------------------

/// Spawns the browser with its own process group and two pipes on descriptors
/// 3 (browser reads) and 4 (browser writes), returning the parent's ends.
#[cfg(unix)]
fn spawn_piped_browser(
    command: &mut Command,
) -> Result<(std::process::Child, File, File), ProbeHarnessError> {
    use std::os::{
        fd::{AsRawFd as _, FromRawFd as _, OwnedFd},
        unix::process::CommandExt as _,
    };
    let make_pipe = || -> Result<(OwnedFd, OwnedFd), ProbeHarnessError> {
        let mut descriptors = [0_i32; 2];
        // SAFETY: `pipe` fills the two-element array it is given.
        if unsafe { libc::pipe(descriptors.as_mut_ptr()) } != 0 {
            return Err(ProbeHarnessError::Launch(format!(
                "could not create a browser protocol pipe: {}",
                std::io::Error::last_os_error()
            )));
        }
        // SAFETY: both descriptors were just created by `pipe` and are owned here.
        let read = unsafe { OwnedFd::from_raw_fd(descriptors[0]) };
        // SAFETY: as above.
        let write = unsafe { OwnedFd::from_raw_fd(descriptors[1]) };
        for descriptor in [read.as_raw_fd(), write.as_raw_fd()] {
            set_close_on_exec(descriptor).map_err(|error| {
                ProbeHarnessError::Launch(format!(
                    "could not make a browser protocol pipe close-on-exec: {error}"
                ))
            })?;
        }
        Ok((read, write))
    };
    // Pipe A: parent writes, browser reads on 3. Pipe B: browser writes on 4,
    // parent reads.
    let (a_read, a_write) = make_pipe()?;
    let (b_read, b_write) = make_pipe()?;
    let a_read_raw = a_read.as_raw_fd();
    let a_write_raw = a_write.as_raw_fd();
    let b_read_raw = b_read.as_raw_fd();
    let b_write_raw = b_write.as_raw_fd();
    command.process_group(0);
    // SAFETY: the closure runs between fork and exec in the child and calls
    // only async-signal-safe functions.
    unsafe {
        command.pre_exec(move || {
            // Move the two child ends onto 3 and 4 without either dup2 clobbering
            // the other's source descriptor.
            let mut source_in = a_read_raw;
            let mut source_out = b_write_raw;
            if source_in == CDP_OUT_DESCRIPTOR {
                source_in = libc::dup(source_in);
                if source_in == -1 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            if source_out == CDP_IN_DESCRIPTOR {
                source_out = libc::dup(source_out);
                if source_out == -1 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            for (source, target) in [
                (source_in, CDP_IN_DESCRIPTOR),
                (source_out, CDP_OUT_DESCRIPTOR),
            ] {
                if source == target {
                    let flags = libc::fcntl(target, libc::F_GETFD);
                    if flags == -1
                        || libc::fcntl(target, libc::F_SETFD, flags & !libc::FD_CLOEXEC) == -1
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                } else if libc::dup2(source, target) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            for descriptor in [
                a_read_raw,
                a_write_raw,
                b_read_raw,
                b_write_raw,
                source_in,
                source_out,
            ] {
                if descriptor != CDP_IN_DESCRIPTOR && descriptor != CDP_OUT_DESCRIPTOR {
                    libc::close(descriptor);
                }
            }
            Ok(())
        });
    }
    let child = command.spawn().map_err(|error| {
        ProbeHarnessError::Launch(format!("could not launch the pinned browser: {error}"))
    })?;
    drop(a_read);
    drop(b_write);
    Ok((child, File::from(a_write), File::from(b_read)))
}

#[cfg(not(unix))]
fn spawn_piped_browser(
    _command: &mut Command,
) -> Result<(std::process::Child, File, File), ProbeHarnessError> {
    Err(ProbeHarnessError::Configuration(
        "this platform cannot give the browser a private protocol pipe".into(),
    ))
}

/// Reads NUL-delimited CDP messages on a detached thread, bounded in size. The
/// thread is never joined, for the same reason the Node report reader is not.
fn spawn_cdp_reader(mut from_browser: File) -> mpsc::Receiver<std::io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::sync_channel::<std::io::Result<Vec<u8>>>(1024);
    std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 64 * 1024];
        loop {
            match from_browser.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    buffer.extend_from_slice(&chunk[..read]);
                    while let Some(position) = buffer.iter().position(|byte| *byte == 0) {
                        let message = buffer.drain(..=position).collect::<Vec<u8>>();
                        let message = message[..message.len() - 1].to_vec();
                        if sender.send(Ok(message)).is_err() {
                            return;
                        }
                    }
                    if buffer.len() > CDP_MAX_MESSAGE_BYTES {
                        let _ = sender.send(Err(std::io::Error::other(
                            "a browser protocol message exceeded its byte budget",
                        )));
                        return;
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                    return;
                }
            }
        }
    });
    receiver
}

struct CdpClient {
    to_browser: File,
    receiver: mpsc::Receiver<std::io::Result<Vec<u8>>>,
    next_id: u64,
    events: VecDeque<serde_json::Value>,
}

impl CdpClient {
    fn new(to_browser: File, from_browser: File) -> Self {
        Self {
            to_browser,
            receiver: spawn_cdp_reader(from_browser),
            next_id: 0,
            events: VecDeque::new(),
        }
    }

    fn receive(&mut self, deadline: Instant) -> Result<serde_json::Value, ProbeHarnessError> {
        let budget = deadline.saturating_duration_since(Instant::now());
        match self.receiver.recv_timeout(budget) {
            Ok(Ok(bytes)) => serde_json::from_slice(&bytes).map_err(|error| {
                ProbeHarnessError::Protocol(format!("invalid browser protocol message: {error}"))
            }),
            Ok(Err(error)) => Err(ProbeHarnessError::Launch(format!(
                "could not read from the browser protocol pipe: {error}"
            ))),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ProbeHarnessError::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ProbeHarnessError::Launch(
                "the browser exited without answering".into(),
            )),
        }
    }

    /// Writes one command and returns its id without waiting for the response.
    fn notify(
        &mut self,
        method: &str,
        params: serde_json::Value,
        session_id: Option<&str>,
    ) -> Result<u64, ProbeHarnessError> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = serde_json::json!({ "id": id, "method": method, "params": params });
        if let Some(session_id) = session_id {
            message["sessionId"] = serde_json::Value::String(session_id.into());
        }
        let mut bytes = serde_json::to_vec(&message)
            .map_err(|error| ProbeHarnessError::Protocol(error.to_string()))?;
        bytes.push(0);
        self.to_browser
            .write_all(&bytes)
            .and_then(|()| self.to_browser.flush())
            .map_err(|error| {
                ProbeHarnessError::Launch(format!(
                    "could not write to the browser protocol pipe: {error}"
                ))
            })?;
        Ok(id)
    }

    /// Writes one command and waits for its response, buffering every event
    /// that arrives meanwhile for [`Self::next_event`]. Never used for a
    /// command whose response depends on an event this loop has yet to answer.
    fn call(
        &mut self,
        method: &str,
        params: serde_json::Value,
        session_id: Option<&str>,
        deadline: Instant,
    ) -> Result<serde_json::Value, ProbeHarnessError> {
        let id = self.notify(method, params, session_id)?;
        loop {
            let message = self.receive(deadline)?;
            if message.get("id").and_then(serde_json::Value::as_u64) == Some(id) {
                if let Some(error) = message.get("error") {
                    return Err(ProbeHarnessError::Protocol(format!(
                        "browser refused {method}: {error}"
                    )));
                }
                return Ok(message
                    .get("result")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null));
            }
            if message.get("method").is_some() {
                self.events.push_back(message);
            }
        }
    }

    fn next_event(&mut self, deadline: Instant) -> Result<serde_json::Value, ProbeHarnessError> {
        if let Some(event) = self.events.pop_front() {
            return Ok(event);
        }
        loop {
            let message = self.receive(deadline)?;
            if message.get("method").is_some() {
                return Ok(message);
            }
        }
    }
}
