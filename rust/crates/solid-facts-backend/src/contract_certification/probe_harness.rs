//! Authority-bearing runtime-probe harness binding for proof policy 2.
//!
//! This module is what makes a nonempty probe-gate schedule executable at all.
//! It mirrors the Type Facts producer discipline in `super::type_facts`: an
//! image identity compiled into the verifier, a private execution directory,
//! *the verified bytes themselves* written into it, a process launched by Rust,
//! and a startup frame that has to echo what Rust computed before the process
//! is trusted with anything.
//!
//! The image is read from disk exactly once. [`verify_harness_image`] returns
//! the bytes it hashed and [`PrivateProbeWorkspace::create`] writes those
//! bytes, so there is no window in which the file on disk can be replaced
//! between the verification and the copy. Re-reading the same paths after
//! verifying would have been such a window.
//!
//! # What is a root of trust here
//!
//! Two digests, both compiled into the verifier by the build:
//!
//! * `SOLID_CHECKER_PROBE_HARNESS_SHA256` — the harness source manifest
//!   (`scripts/probe-harness-source-identity.mjs`). The harness is
//!   interpreted, so its source *is* its executable image; the manifest is
//!   recomputed here from the bytes on disk, not read from a stamp.
//! * `SOLID_CHECKER_PROBE_NODE_SHA256` — the sha256 of the regular-file bytes
//!   of the Node executable this build was made to launch.
//!
//! A build without both refuses probe authority exactly as
//! [`super::TypeFactsProducerPin::configured`] does. Nothing is read from a
//! runtime string, an environment variable, or an adjacent stamp as its own
//! root: the stamp beside the CLI is verified *against* the compiled-in
//! manifest digest, and disagreeing with it is a refusal.
//!
//! # What the worker is allowed to say
//!
//! Nothing semantic. The worker's reported runtime, isolation, and environment
//! fields are transport data that must *equal* what Rust computed for the
//! session; a worker-reported digest is never trusted, and the worker's
//! observed Node version, platform, and architecture have to match the version
//! the pinned bytes reported and the platform this verifier is running on. The
//! recipe's construction digest is taken by Rust from the module bytes it
//! copied, so the JavaScript-side check in `contract-probe-driver.mjs` is a
//! convenience for the audit path and no longer the enforcer.
//!
//! # Which file the probe ran against
//!
//! A gate that observed the wrong artifact case is a false *pass*, and the
//! export-condition set is what chooses the file. `plan.import_request`'s
//! conditions select the artifact case the gate subject names and the Type
//! Facts witness reads; the interpreter applies its own set on top of the flags
//! it is given, and on the pinned Node that set includes `module-sync` and
//! `node-addons` and swaps `import` for `require` on a `createRequire`. A
//! package with a conforming `module-sync` target beside a contradicting
//! `import` one would therefore be certified on one file and probed against
//! another.
//!
//! Three mechanisms, only the last of which is load-bearing:
//!
//! * a recipe **declares** its import kind, because the two kinds resolve
//!   differently, and Rust binds the declared kind into the corpus root;
//! * every requested condition is passed as a `--conditions=` flag, and the
//!   conditions the interpreter *actually applies* are read back from the
//!   pinned bytes per kind ([`observe_conditions`]) and recorded tagged in the
//!   environment identity — never a constant;
//! * the worker reports what the plan's specifier resolves to, both ways,
//!   before the recipe is imported, and [`verify_reported_resolution`] refuses
//!   unless the answer for the declared kind names the exact runtime target the
//!   witness read. [`refuse_unreproducible_artifact_case`] refuses the same
//!   mismatch at planning time, before anything is copied.
//!
//! # Why the report arrives on its own descriptor
//!
//! The recipe imports the analyzed package into the worker's realm, so package
//! top-level code runs before any frame is written and can replace anything the
//! report path reaches by name. `process.stdout.write`, `JSON.stringify`,
//! `Array.prototype.push`, and `structuredClone` were all reachable, and
//! patching `structuredClone` alone was enough to drop an event and renumber
//! the rest into a clean-looking transcript.
//!
//! Three things answer that. The JavaScript side captures every primordial the
//! report path needs into module-local bindings before the recipe is imported
//! (`contract-probe-harness.mjs`, `contract-probe-worker.mjs`). Capturing
//! `JSON.stringify` was *not* one of the answers, because the algorithm
//! performs `Get(value, "toJSON")` on every object it visits: a package
//! defining `Object.prototype.toJSON` — or `Array.prototype.toJSON`, for the
//! events container — was handed the real run frame and could return a
//! laundered one, so the whole frame is built as null-prototype records and
//! serialized by the harness's own serializer, and the worker freezes the
//! intrinsic prototypes before importing the recipe. And the frames travel on
//! descriptor 3, a pipe this module creates and hands to the child, never on
//! stdout: the worker's stdout is `/dev/null`, so package code writing there
//! reaches nothing. Rust then requires *exactly* the startup frame and one run
//! frame on that descriptor — an extra frame is a refusal rather than a choice
//! of which one to believe.
//!
//! The descriptor number is not a secret, and nothing here pretends otherwise:
//! in-realm code can name it (`fs.writeSync(3, …)`), a native addon runs
//! outside every JavaScript guarantee, and a child process inherits it unless
//! the spawner closes it. What package code cannot reach is *the harness's own
//! write binding by name*, and what refuses a forged frame is that a frame has
//! to carry this launch's nonce and name the pid Rust launched, with a third
//! frame refused outright. So an in-realm writer can spoil a run — a refusal —
//! but cannot substitute a laundered one.
//!
//! # Write isolation: detect and refuse, not deny
//!
//! Policy 2 requires the probe to run "in a sandbox that denies writes to
//! snapshot and producer/compiler inputs" or refuse. Stage 1 implements the
//! property that requirement protects — that no probe run can have altered the
//! inputs the rest of the transaction reads — without an OS-level sandbox:
//!
//! * the probe runs against a *private copy* of the artifact snapshot inside a
//!   0700 directory, never the shared materialized store or the analyzed tree.
//!   The **authenticated dependency closure** is copied beside it, under the
//!   same discipline: only snapshots this certification transaction already
//!   authenticated ([`authenticated_dependency_closure`]), never the project's
//!   real `node_modules` and never a registry. A dependency the analyzed
//!   package imports and this transaction did not authenticate refuses the gate
//!   by name ([`require_authenticated_dependency_closure`]) instead of the
//!   probe reaching unauthenticated bytes, and two authenticated versions of
//!   one name refuse rather than one being chosen between;
//! * the private directory itself is censused by its *direct entries*, because
//!   `TMPDIR` and the cwd are that directory and a `node.config.json` landing
//!   there has to be a change; the cost is that a probe writing a temporary
//!   file into `TMPDIR` refuses the gate;
//! * digests of the whole private `node_modules` tree, each dependency copy
//!   under its own label, the copied harness
//!   image, the copied recipe modules, the two CommonJS "global folders" a
//!   `HOME` inside the private directory would make resolvable, every
//!   `<ancestor>/node_modules` a bare specifier could reach by walking up, the
//!   Node executable, the Type Facts producer image, and this verifier's own
//!   image are recorded before the first launch;
//! * they are re-hashed **between** launches as well as after the last one, so
//!   one session cannot tamper with what the next reads and restore it before
//!   the final census, and the census runs on every exit path including a
//!   launch error or timeout;
//! * the one watched path with an expected value known in advance — the Node
//!   executable — is compared against the *pin* on every census rather than
//!   against the baseline, because two censuses of a binary that was already
//!   substituted agree with each other;
//! * any change — a new file, a watched file replaced by a symlink, or a
//!   watched path that was absent and now exists — refuses the gate with
//!   [`ProbeHarnessError::IsolationViolation`].
//!
//! Resolution is contained as well as watched, step by step against Node's own
//! algorithm rather than one patch per discovered escape — the disposition
//! table in `docs/adr/0006-probe-harness-binding.md` is the enumeration, and
//! [`sandbox_policy_digest`] carries its field names. The three steps that
//! reach outside the private tree:
//!
//! * `PACKAGE_SELF_RESOLVE` and `PACKAGE_IMPORTS_RESOLVE` both start at
//!   `LOOKUP_PACKAGE_SCOPE`, which climbs to the *first* `package.json` above
//!   the importer — `<tmpdir>/package.json` without a nearer one, and that is a
//!   world-writable location on Linux. A planted `{"name": "<the analyzed
//!   package>", "exports": …}` there wins over the private snapshot copy and
//!   the recipe observes a stub: a false *pass*. [`create_private_layout`]
//!   therefore writes an `exports`-less, `imports`-less package scope into
//!   `<private>/harness/` and `<private>/recipes/`, which ends the climb inside
//!   the 0700 tree.
//! * the `node_modules` walk goes *up* to `<tmpdir>/node_modules` and
//!   `/node_modules`, so a private directory with any of those above it refuses
//!   before a launch happens.
//! * the CommonJS global folders. Two are `HOME`-relative and `HOME` is the
//!   private directory; the third, `$PREFIX/lib/node`, comes from the Node
//!   executable's install prefix rather than the environment, so it is refused
//!   and watched like an ancestor `node_modules`.
//!
//! Because each precondition is a point-in-time check on a shared tree, every
//! candidate it cleared — and every ancestor `package.json` the private scopes
//! now shadow — joins the watched census afterwards.
//!
//! This is detection, not denial. A run that writes somewhere none of these
//! digests covers is not prevented, and **network access is not denied**.
//! Containment is bounded by the process group as well: a probe that calls
//! `setsid()` leaves the group this module kills, and such a grandchild
//! outliving the transaction could disturb a watched path after the last
//! census — undetected. Stage 2 replaces this with an OS-level sandbox; see
//! `docs/adr/0006-probe-harness-binding.md`.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufRead as _, BufReader, Read as _, Write as _},
    path::{Component, Path, PathBuf},
    process::{Child, ChildStderr, Command, Stdio},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{Digest, SemanticClaimPath, SemanticClaimSubject};
use thiserror::Error;

use super::{CertificationPlan, TypeFactsProducerPin, probe_gates::ProbeGateSchedule};
use crate::{
    EnvironmentIdentity, SandboxIdentity, SandboxKind, ToolIdentity,
    runtime_probe_wire::{self, ReportedResolution, RuntimeProbeWireError},
    runtime_probes::{
        ArtifactModeMatrix, DrainStep, ProbeAuthority, ProbeEventClass, ProbeEventMatch, ProbeMode,
        ProbePolicy, ProbeRecipe, ProbeRun, ProbeScenario, RuntimeProbeError,
        RuntimeProbeEvaluation, RuntimeProbePlan, evaluate_runtime_probes,
    },
};

/// The startup/frame protocol Rust and the worker agree on. Bumping this
/// invalidates every harness manifest digest, because the worker carries the
/// string too.
pub(crate) const PROBE_WORKER_PROTOCOL: &str = "solid-checker-runtime-probe-v5";

const STARTUP_FORMAT: &str = "solid-checker-probe-worker-startup";
const RECIPE_CORPUS_FORMAT: &str = "solid-checker-probe-recipe-corpus";
const RECIPE_CORPUS_MANIFEST: &str = "recipes.json";
const RECIPE_CORPUS_SCHEMA_VERSION: u16 = 1;
const MAX_CORPUS_BYTES: usize = 1024 * 1024;
const MAX_RECIPE_BYTES: usize = 1024 * 1024;
/// Total bytes one launch may write on the report descriptor, across every
/// frame. The reader stops at the cap instead of buffering whatever arrives.
const MAX_REPORT_BYTES: usize = 16 * 1024 * 1024;
/// Frames one launch may write: the startup frame and one run frame. A third is
/// a protocol refusal, never a choice of which frame to believe.
const MAX_REPORT_FRAMES: usize = 2;
/// Bytes of the worker's stderr kept for diagnostics. Nothing semantic is read
/// from it; it exists so a launch failure says something more than "exited".
const MAX_STDERR_BYTES: usize = 64 * 1024;
const STARTUP_BUDGET: Duration = Duration::from_secs(30);
/// How long a killed worker's already-written extra frame is waited for. The
/// process group is dead by then, so this only covers the reader thread's
/// scheduling, not the worker's willingness to answer.
const EXTRA_FRAME_GRACE: Duration = Duration::from_millis(250);
/// The descriptor the worker reports on. Not stdout: package top-level code
/// runs in the worker's realm and can replace `process.stdout.write`.
const REPORT_DESCRIPTOR: i32 = 3;
/// The digest recorded for a watched path that does not exist. A path that
/// later exists hashes to something else, so appearing is a violation.
const ABSENT_WATCHED_PATH: &str = "absent";
/// The bytes written as `<private>/harness/package.json` and
/// `<private>/recipes/package.json`.
///
/// No `exports`, no `imports`, no `main`: those three fields are the whole
/// reason a package scope can answer a specifier at all, and this manifest
/// exists only to *be* the nearest scope so that `LOOKUP_PACKAGE_SCOPE`
/// terminates inside the 0700 tree. `type` is declared so a stray `.js` in
/// either directory is an ES module rather than a format decided by an
/// ancestor. The `name` is deliberately neither the analyzed package's nor one
/// any recipe writes as a bare specifier, so even a future manifest here that
/// grew an `exports` map would have nothing to answer.
const PRIVATE_PACKAGE_SCOPE_MANIFEST: &[u8] =
    b"{\n  \"name\": \"solid-checker-probe-private\",\n  \"type\": \"module\",\n  \"private\": true\n}\n";
const PRIVATE_PACKAGE_SCOPE_MANIFEST_NAME: &str = "package.json";
const HARNESS_MODULES: [&str; 2] = [
    "packages/cli/scripts/contract-probe-harness.mjs",
    "packages/cli/scripts/contract-probe-worker.mjs",
];
pub(crate) const HARNESS_STAMP: &str = "packages/cli/probe-harness.buildinfo";
const WORKER_ENTRY: &str = "contract-probe-worker.mjs";

/// The conditions the pinned interpreter is *asked about* when the harness
/// records which ones it actually applies.
///
/// The answer comes from the interpreter; this list is only the question, so a
/// condition Node applies that is absent here is missing from the record. It
/// cannot become a false *pass*: the reported resolution still has to name the
/// artifact case's own runtime target ([`verify_reported_resolution`]), so an
/// unlisted condition that changes the selected file refuses the gate.
const OBSERVED_CONDITION_CANDIDATES: [&str; 16] = [
    "browser",
    "bun",
    "deno",
    "development",
    "electron",
    "import",
    "module",
    "module-sync",
    "node",
    "node-addons",
    "production",
    "react-native",
    "react-server",
    "require",
    "types",
    "worker",
];

/// The `node_modules` directory name each condition candidate gets inside the
/// observation directory.
const CONDITION_PACKAGE_PREFIX: &str = "solid-checker-probe-condition-";

/// Asks one interpreter which export conditions it applies, per import kind.
///
/// Rust-authored bytes only: it imports nothing of any analyzed package, and
/// it runs before the private workspace exists, so it takes no watched
/// census. Each candidate gets a package whose `exports` maps that one
/// condition to `applied.mjs` and everything else to `unapplied.mjs`, and the
/// interpreter's own resolver decides which file each request lands on.
const CONDITION_OBSERVER: &str = r#"import { createRequire } from "node:module";
const cjs = createRequire(import.meta.url);
const resolveModule = import.meta.resolve;
const esm = [];
const required = [];
for (const condition of process.argv.slice(2)) {
  const specifier = `solid-checker-probe-condition-${condition}`;
  try {
    if (resolveModule(specifier).endsWith("/applied.mjs")) esm.push(condition);
  } catch {}
  try {
    if (cjs.resolve(specifier).endsWith("/applied.mjs")) required.push(condition);
  } catch {}
}
process.stdout.write(`esm:${esm.join(",")}\nrequire:${required.join(",")}\n`);
"#;

/// How a recipe reaches the package under test.
///
/// A recipe declares this because the two paths do not resolve alike: the
/// export-condition set Node applies to an `import` is not the one it applies
/// to a `require`, so which file a bare specifier lands on depends on the
/// kind. Rust binds the declared kind into the plan and checks the resolution
/// that kind actually produced.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ProbeImportKind {
    /// `import … from "<package>"`, static or dynamic.
    Esm,
    /// `createRequire(…)("<package>")`.
    Require,
}

impl ProbeImportKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Esm => "esm",
            Self::Require => "require",
        }
    }
}

/// What one launch has to prove it resolved: the plan's own specifier, reached
/// the way the recipe declares it reaches its package, plus every dependency
/// specifier the recipe declared it needs.
#[derive(Clone, Copy, Debug)]
struct ResolutionSubject<'a> {
    specifier: &'a str,
    import_kind: ProbeImportKind,
    /// The recipe's declared dependency specifiers, each of which must resolve
    /// *inside* that dependency's authenticated private copy.
    dependencies: &'a [String],
}

/// The closed, sorted file list the harness source manifest covers. It must
/// stay byte-identical to `HARNESS_FILES` in
/// `scripts/probe-harness-source-identity.mjs`; a divergence makes every
/// launch refuse rather than trust the wrong bytes.
pub(crate) const HARNESS_MANIFEST_FILES: [&str; 8] = [
    "packages/cli/bun.lock",
    "packages/cli/package-lock.json",
    "packages/cli/package.json",
    "packages/cli/scripts/contract-probe-driver.mjs",
    "packages/cli/scripts/contract-probe-harness.mjs",
    "packages/cli/scripts/contract-probe-worker.mjs",
    "packages/cli/scripts/probe-contract.mjs",
    "scripts/probe-harness-source-identity.mjs",
];

static PRIVATE_HARNESS_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Where the harness image, the Node executable, and the hand-authored recipe
/// corpus live for one certification transaction.
///
/// Every path is absolute and supplied by the operator, never discovered
/// inside the analyzed package: a recipe corpus that sits under the package
/// root is refused, because a package would then be choosing the probe that
/// vetoes its own proposed closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeHarnessConfiguration {
    harness_root: PathBuf,
    node_executable: PathBuf,
    recipe_corpus: PathBuf,
    /// A pin supplied by an in-crate test instead of the build.
    ///
    /// Private, and settable only through a `#[cfg(test)]` constructor, so a
    /// release build has no path to it and still refuses without the
    /// compiled-in digests. Tests compute the same digests from the same
    /// files the build would, which is what lets the fixture tracer run under
    /// a plain `cargo test`.
    #[cfg(test)]
    pin: Option<ProbeHarnessPin>,
}

impl ProbeHarnessConfiguration {
    pub fn new(
        harness_root: impl Into<PathBuf>,
        node_executable: impl Into<PathBuf>,
        recipe_corpus: impl Into<PathBuf>,
    ) -> Result<Self, ProbeHarnessError> {
        let configuration = Self {
            harness_root: harness_root.into(),
            node_executable: node_executable.into(),
            recipe_corpus: recipe_corpus.into(),
            #[cfg(test)]
            pin: None,
        };
        for (label, path) in [
            ("probe harness root", &configuration.harness_root),
            ("probe Node executable", &configuration.node_executable),
            ("probe recipe corpus", &configuration.recipe_corpus),
        ] {
            if !path.is_absolute() {
                return Err(ProbeHarnessError::Configuration(format!(
                    "{label} must be an absolute path"
                )));
            }
        }
        Ok(configuration)
    }

    /// The directory holding `recipes.json` and its modules — the corpus
    /// recipe-gated planning consults before any demand is derived.
    #[must_use]
    pub fn recipe_corpus(&self) -> &Path {
        &self.recipe_corpus
    }

    /// The pin this transaction verifies against: the build's, unless an
    /// in-crate test supplied its own.
    fn pin(&self) -> Result<ProbeHarnessPin, ProbeHarnessError> {
        #[cfg(test)]
        if let Some(pin) = &self.pin {
            return Ok(pin.clone());
        }
        ProbeHarnessPin::configured()
    }

    #[cfg(test)]
    pub(crate) fn with_test_pin(
        harness_root: impl Into<PathBuf>,
        node_executable: impl Into<PathBuf>,
        recipe_corpus: impl Into<PathBuf>,
        harness_manifest_sha256: &str,
        node_executable_sha256: &str,
    ) -> Result<Self, ProbeHarnessError> {
        let mut configuration = Self::new(harness_root, node_executable, recipe_corpus)?;
        configuration.pin = Some(ProbeHarnessPin::new(
            harness_manifest_sha256,
            node_executable_sha256,
        )?);
        Ok(configuration)
    }
}

/// The digests this verifier build was made to launch.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ProbeHarnessPin {
    harness_manifest_sha256: String,
    node_executable_sha256: String,
}

impl ProbeHarnessPin {
    /// Loads the harness identity compiled into this verifier build.
    ///
    /// A development build without both values refuses probe authority instead
    /// of trusting runtime strings or an adjacent stamp as its own root of
    /// trust. The consequence is deliberate: on such a build a nonempty probe
    /// schedule cannot certify at all — it never silently certifies without
    /// the veto.
    fn configured() -> Result<Self, ProbeHarnessError> {
        let harness_manifest_sha256 = option_env!("SOLID_CHECKER_PROBE_HARNESS_SHA256")
            .ok_or_else(|| {
                ProbeHarnessError::PinUnavailable(
                    "verifier build has no configured probe harness source-manifest digest",
                )
            })?;
        let node_executable_sha256 =
            option_env!("SOLID_CHECKER_PROBE_NODE_SHA256").ok_or_else(|| {
                ProbeHarnessError::PinUnavailable(
                    "verifier build has no configured probe Node executable digest",
                )
            })?;
        Self::new(harness_manifest_sha256, node_executable_sha256)
    }

    fn new(
        harness_manifest_sha256: &str,
        node_executable_sha256: &str,
    ) -> Result<Self, ProbeHarnessError> {
        for (label, value) in [
            ("harness source-manifest", harness_manifest_sha256),
            ("Node executable", node_executable_sha256),
        ] {
            if Digest::parse(value).is_err() {
                return Err(ProbeHarnessError::Configuration(format!(
                    "configured probe {label} digest is not a canonical sha256 digest"
                )));
            }
        }
        Ok(Self {
            harness_manifest_sha256: harness_manifest_sha256.to_owned(),
            node_executable_sha256: node_executable_sha256.to_owned(),
        })
    }
}

/// This build's compiled-in pins, for tests that want to exercise the
/// production path rather than their own. `None` on a development build,
/// which is exactly the build that must refuse probe authority.
#[cfg(test)]
pub(crate) fn configured_pin_digests() -> Option<(String, String)> {
    ProbeHarnessPin::configured()
        .ok()
        .map(|pin| (pin.harness_manifest_sha256, pin.node_executable_sha256))
}

/// Proof that a specific harness image and Node runtime executed a specific
/// gate schedule, with the snapshot copy and every producer image unchanged
/// across the run.
///
/// Non-serializable, `pub(crate)`, and produced only by [`run_probe_gates`].
/// [`ProbeGateSchedule::authenticate_with_harness`] refuses one bound to any
/// other snapshot, demand graph, or gate set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BoundProbeHarnessIdentity {
    snapshot_root: String,
    demand_graph_root: String,
    gate_ids: Vec<String>,
    execution: HarnessExecution,
    /// Stable identity fields bound into the receipt's probe-gate root, in
    /// this exact order. Per-launch nonces, process ids, and launch epochs are
    /// deliberately absent: they bind the live worker inside the transaction,
    /// while a receipt root has to stay reproducible from the same inputs.
    fields: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HarnessExecution {
    PublishedBytes,
    ControlledInert,
}

impl BoundProbeHarnessIdentity {
    pub(crate) fn requires_controlled_execution(&self) -> bool {
        self.execution != HarnessExecution::PublishedBytes
    }

    pub(crate) fn snapshot_root(&self) -> &str {
        &self.snapshot_root
    }

    pub(crate) fn demand_graph_root(&self) -> &str {
        &self.demand_graph_root
    }

    pub(crate) fn gate_ids(&self) -> &[String] {
        &self.gate_ids
    }

    pub(crate) fn root_fields(&self) -> Vec<&str> {
        self.fields.iter().map(String::as_str).collect()
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        snapshot_root: &str,
        demand_graph_root: &str,
        gate_ids: Vec<String>,
    ) -> Self {
        Self {
            snapshot_root: snapshot_root.into(),
            demand_graph_root: demand_graph_root.into(),
            gate_ids,
            execution: HarnessExecution::PublishedBytes,
            fields: vec![
                "harness-manifest:sha256:test".into(),
                "node-executable:sha256:test".into(),
            ],
        }
    }
}

/// Executes every mandatory probe gate of `schedule` inside this
/// certification transaction and returns the evaluation together with the
/// harness identity that ran it.
///
/// Probe execution happens only here — never during analysis, proposal
/// generation, or catalog discovery.
pub(crate) fn run_probe_gates(
    plan: &CertificationPlan,
    schedule: &ProbeGateSchedule,
    configuration: &ProbeHarnessConfiguration,
    type_facts_pin: &TypeFactsProducerPin,
    graph_dependencies: &[&CertificationPlan],
) -> Result<(RuntimeProbeEvaluation, BoundProbeHarnessIdentity), ProbeHarnessError> {
    run_probe_gates_inner(
        plan,
        schedule,
        configuration,
        type_facts_pin,
        graph_dependencies,
        None,
    )
}

pub(super) fn run_inert_gates(
    plan: &CertificationPlan,
    schedule: &ProbeGateSchedule,
    configuration: &ProbeHarnessConfiguration,
    type_facts_pin: &TypeFactsProducerPin,
    module: &super::controlled_execution::InertModule,
    consumer: bool,
) -> Result<(RuntimeProbeEvaluation, BoundProbeHarnessIdentity), ProbeHarnessError> {
    run_probe_gates_inner(
        plan,
        schedule,
        configuration,
        type_facts_pin,
        &[],
        Some((module, consumer)),
    )
}

fn run_probe_gates_inner(
    plan: &CertificationPlan,
    schedule: &ProbeGateSchedule,
    configuration: &ProbeHarnessConfiguration,
    type_facts_pin: &TypeFactsProducerPin,
    graph_dependencies: &[&CertificationPlan],
    inert: Option<(&super::controlled_execution::InertModule, bool)>,
) -> Result<(RuntimeProbeEvaluation, BoundProbeHarnessIdentity), ProbeHarnessError> {
    if schedule.gates().is_empty() {
        return Err(ProbeHarnessError::Configuration(
            "an empty probe schedule needs no harness launch".into(),
        ));
    }
    let pin = configuration.pin()?;
    let node = verify_node_executable(&configuration.node_executable, &pin)?;
    let image = verify_harness_image(&configuration.harness_root, &pin)?;
    let node_version = node_version(&configuration.node_executable, &node)?;

    let corpus = RecipeCorpus::load(&configuration.recipe_corpus, plan)?;
    // What this plan's artifact case was resolved under, and what the pinned
    // interpreter actually applies. The second is measured, never assumed: the
    // two are not the same set, and the difference is what selects a file.
    let requested = requested_conditions(plan)?;
    let observed = observe_conditions(&configuration.node_executable, &node, &requested)?;
    let environment = probe_environment(&node, &node_version, &requested, &observed);
    let runtime_plan = plan.runtime_probe_plan(schedule, &corpus, environment.clone())?;
    // Before anything is copied or launched: this interpreter has to select the
    // very runtime target the Type Facts witness read, for every import kind
    // the scheduled recipes declare. A conforming `module-sync` target beside a
    // contradicting `import` one is otherwise a false pass.
    for kind in scheduled_import_kinds(schedule, &corpus)? {
        if inert.is_some() && kind != ProbeImportKind::Esm {
            return Err(ProbeHarnessError::Configuration(
                "inert execution requires ESM recipes".into(),
            ));
        }
        refuse_unreproducible_artifact_case(plan, kind, &observed)?;
    }
    // Static graph edges are ESM imports. Reproduce their independently
    // planned targets too; an extra Node condition must not silently select a
    // different dependency artifact case from the same authenticated archive.
    for dependency in graph_dependencies {
        refuse_unreproducible_artifact_case(dependency, ProbeImportKind::Esm, &observed)?;
    }
    // The dependency closure the private workspace will carry, and the refusal
    // that keeps it from being a partial one. Both happen before the private
    // directory exists: a dependency the analyzed package imports and this
    // transaction did not authenticate is a named refusal, never a probe
    // against whatever the layout happens to resolve.
    let dependencies = authenticated_dependency_closure(plan, graph_dependencies)?;
    require_authenticated_dependency_closure(plan, &dependencies)?;
    for dependency in graph_dependencies {
        require_authenticated_dependency_closure(dependency, &dependencies)?;
    }
    require_declared_dependencies_authenticated(schedule, &corpus, &dependencies)?;

    let workspace_inputs = PrivateWorkspaceInputs {
        plan,
        image: &image,
        corpus: &corpus,
        node_executable: &configuration.node_executable,
        // The pin's own value for the Node bytes, so the watched census
        // *re-asserts* it rather than recording whatever is on disk when the
        // baseline is taken. The pin check above and the first launch below are
        // separate reads of the same path.
        node_executable_sha256: &node,
        type_facts_pin,
        requested_conditions: &requested,
        dependencies: &dependencies,
    };
    let workspace = if inert.is_some() {
        PrivateProbeWorkspace::create_with_inert(&workspace_inputs, inert)?
    } else {
        PrivateProbeWorkspace::create(&workspace_inputs)?
    };

    let launched = launch_every_session(
        &workspace,
        &corpus,
        &runtime_plan,
        &configuration.node_executable,
        &node_version,
        &plan.resolved_import.specifier,
    );
    // The census runs on *every* exit path, a launch failure or timeout
    // included: a run that altered a producer input has to refuse the gate
    // rather than be reported as a mere launch problem, and an isolation
    // violation is the more serious of the two facts.
    let runs = match (launched, workspace.verify_unchanged()) {
        (_, Err(violation)) => return Err(violation),
        (Err(error), Ok(())) => return Err(error),
        (Ok(runs), Ok(())) => runs,
    };

    let producer = ToolIdentity {
        name: "solid-checker-probe-harness".into(),
        version: node_version.clone(),
        build: Digest::parse(pin.harness_manifest_sha256.clone())
            .expect("the configured harness manifest digest was validated"),
        protocol: Some(PROBE_WORKER_PROTOCOL.into()),
    };
    let evaluation = evaluate_runtime_probes(&runtime_plan, runs, producer)?;
    let mut identity = BoundProbeHarnessIdentity {
        execution: if inert.is_some() {
            HarnessExecution::ControlledInert
        } else {
            HarnessExecution::PublishedBytes
        },
        snapshot_root: plan.snapshot.root().to_owned(),
        demand_graph_root: plan.demand_graph.root().as_str().to_owned(),
        gate_ids: schedule
            .gates()
            .iter()
            .map(|gate| gate.id().to_owned())
            .collect(),
        fields: vec![
            format!("harness-manifest:{}", pin.harness_manifest_sha256),
            format!("node-executable:{}", pin.node_executable_sha256),
            format!("node-version:{node_version}"),
            format!("worker-protocol:{PROBE_WORKER_PROTOCOL}"),
            format!("sandbox-policy:{}", sandbox_policy_digest().as_str()),
            format!("runtime-probe-plan:{}", runtime_plan.digest().as_str()),
            format!("recipe-corpus:{}", corpus.root.as_str()),
            format!(
                "dependency-materialization:{}",
                dependency_materialization_root(&dependencies)
            ),
        ],
    };
    if let Some((module, _)) = inert {
        identity.fields.extend(module.binding());
    }
    Ok((evaluation, identity))
}

/// Runs one launch per planned session, re-hashing the watched census between
/// launches.
///
/// Launches are sequential, so without the intermediate census session N could
/// tamper with what session N+1 reads and restore it before the final check.
fn launch_every_session(
    workspace: &PrivateProbeWorkspace,
    corpus: &RecipeCorpus,
    runtime_plan: &RuntimeProbePlan,
    node_executable: &Path,
    node_version: &str,
    specifier: &str,
) -> Result<Vec<ProbeRun>, ProbeHarnessError> {
    let mut runs = Vec::with_capacity(runtime_plan.sessions().len());
    for session in runtime_plan.sessions() {
        let claim_id = session.claim_id().as_str();
        let recipe = corpus.recipe_for(claim_id).ok_or_else(|| {
            ProbeHarnessError::RecipeProvenance(format!(
                "the recipe corpus stopped naming claim {claim_id} mid-transaction"
            ))
        })?;
        let (module, relative) = workspace.recipe(claim_id).ok_or_else(|| {
            ProbeHarnessError::RecipeProvenance(format!(
                "no private recipe module was copied for claim {claim_id}"
            ))
        })?;
        let subject = ResolutionSubject {
            specifier,
            import_kind: recipe.import_kind(),
            dependencies: recipe.dependency_specifiers(),
        };
        let session_bytes = runtime_probe_wire::encode_probe_session(
            session,
            relative,
            recipe.construction(),
            Some(runtime_probe_wire::ProbeResolutionRequest {
                specifier: subject.specifier,
                import_kind: subject.import_kind.as_str(),
                dependencies: subject.dependencies,
            }),
        )?;
        let run = workspace.launch(
            node_executable,
            node_version,
            module,
            &session_bytes,
            session.policy().timeout_millis,
            subject,
        )?;
        workspace.verify_unchanged()?;
        runs.push(run);
    }
    Ok(runs)
}

/// Every distinct import kind the scheduled gates' recipes declare.
///
/// A gate with no recipe is [`ProbeHarnessError::MissingRecipe`] here for the
/// same reason it is in [`runtime_probe_plan`]: a scheduled veto that cannot
/// run refuses rather than being skipped.
fn scheduled_import_kinds(
    schedule: &ProbeGateSchedule,
    corpus: &RecipeCorpus,
) -> Result<BTreeSet<ProbeImportKind>, ProbeHarnessError> {
    schedule
        .gates()
        .iter()
        .map(|gate| {
            corpus
                .recipe_for(gate.semantic_claim_id())
                .map(CorpusRecipe::import_kind)
                .ok_or_else(|| ProbeHarnessError::MissingRecipe {
                    gate_id: gate.id().to_owned(),
                    semantic_claim_id: gate.semantic_claim_id().to_owned(),
                })
        })
        .collect()
}

/// Refuses a scheduled recipe that declares a dependency specifier this
/// transaction authenticated no snapshot for.
///
/// [`require_authenticated_dependency_closure`] covers what the *package*
/// imports; this covers what a *recipe* says it needs. The two are different
/// claims and both refuse by name: a recipe may legitimately name a dependency
/// the closure replay did not record an edge for (a peer the package reaches
/// only through a re-export chain, say), and a recipe naming one that was never
/// authenticated would otherwise ask the worker to resolve a specifier no
/// private copy answers and refuse as a resolution mismatch instead of by name.
fn require_declared_dependencies_authenticated(
    schedule: &ProbeGateSchedule,
    corpus: &RecipeCorpus,
    closure: &BTreeMap<String, AuthenticatedDependency<'_>>,
) -> Result<(), ProbeHarnessError> {
    for gate in schedule.gates() {
        let recipe = corpus.recipe_for(gate.semantic_claim_id()).ok_or_else(|| {
            ProbeHarnessError::MissingRecipe {
                gate_id: gate.id().to_owned(),
                semantic_claim_id: gate.semantic_claim_id().to_owned(),
            }
        })?;
        for specifier in recipe.dependency_specifiers() {
            if closure.contains_key(specifier) {
                continue;
            }
            return Err(ProbeHarnessError::CorpusInvalid(format!(
                "probe gate {} declares dependency specifier {specifier:?}, which this \
                 certification transaction authenticated no snapshot for",
                gate.id()
            )));
        }
    }
    Ok(())
}

/// The export conditions this plan's artifact case was selected under.
///
/// They reach the harness from the certification request, so each one is
/// validated before it is interpolated into a `--conditions=` flag or into the
/// observation packages' manifests. A name outside the conservative charset
/// refuses rather than being passed through.
fn requested_conditions(plan: &CertificationPlan) -> Result<Vec<String>, ProbeHarnessError> {
    let mut conditions = Vec::new();
    for condition in &plan.import_request.export_conditions {
        plain_condition_name(condition)?;
        conditions.push(condition.clone());
    }
    conditions.sort();
    conditions.dedup();
    Ok(conditions)
}

/// One plain export-condition name, refused unless it is safe to interpolate
/// into an interpreter flag and into a package manifest.
fn plain_condition_name(condition: &str) -> Result<(), ProbeHarnessError> {
    if condition.is_empty()
        || condition.len() > 64
        || !condition.bytes().all(
            |byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_'),
        )
    {
        return Err(ProbeHarnessError::Configuration(format!(
            "export condition {condition:?} is not a plain condition name, so it cannot be given \
             to the pinned interpreter"
        )));
    }
    Ok(())
}

/// The export conditions the pinned interpreter *actually applies*, per import
/// kind.
///
/// Nothing here is a constant. `["import", "node"]` was one, and it was wrong:
/// the pinned Node 24.11.1 also applies `module-sync` and `node-addons` to an
/// `import`, and `require` rather than `import` to a `createRequire` — and a
/// package that lists `module-sync` first is answered with that target for
/// *both*.
struct ObservedConditions {
    esm: Vec<String>,
    require: Vec<String>,
}

impl ObservedConditions {
    fn for_kind(&self, kind: ProbeImportKind) -> &[String] {
        match kind {
            ProbeImportKind::Esm => &self.esm,
            ProbeImportKind::Require => &self.require,
        }
    }
}

/// Asks the pinned bytes which export conditions they apply, under exactly the
/// `--conditions` flags a launch will carry.
///
/// The measurement is cached by the Node digest and the requested set, because
/// it is a property of those two and of nothing else in the transaction.
fn observe_conditions(
    node_executable: &Path,
    node_sha256: &str,
    requested: &[String],
) -> Result<ObservedConditions, ProbeHarnessError> {
    /// One measured answer per (Node digest, requested set), as the two
    /// per-kind sets it consists of.
    type ObservedConditionCache = BTreeMap<String, (Vec<String>, Vec<String>)>;
    static CACHE: OnceLock<std::sync::Mutex<ObservedConditionCache>> = OnceLock::new();
    let key = format!("{node_sha256}\u{1f}{}", requested.join("\u{1f}"));
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(BTreeMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some((esm, require)) = guard.get(&key)
    {
        return Ok(ObservedConditions {
            esm: esm.clone(),
            require: require.clone(),
        });
    }
    let mut candidates = OBSERVED_CONDITION_CANDIDATES
        .into_iter()
        .collect::<BTreeSet<_>>();
    for condition in requested {
        candidates.insert(condition.as_str());
    }
    let candidates = candidates.into_iter().collect::<Vec<_>>();
    // Its own private directory, and removed again before the workspace is
    // created: the observation tree carries a `node_modules`, and a workspace
    // with one *above* it refuses by design.
    let directory = create_private_directory("conditions")?;
    let observed = observe_conditions_in(&directory, node_executable, requested, &candidates);
    let _ = remove_private_tree(&directory);
    let (esm, require) = observed?;
    if let Ok(mut guard) = cache.lock() {
        guard.insert(key, (esm.clone(), require.clone()));
    }
    Ok(ObservedConditions { esm, require })
}

fn observe_conditions_in(
    directory: &Path,
    node_executable: &Path,
    requested: &[String],
    candidates: &[&str],
) -> Result<(Vec<String>, Vec<String>), ProbeHarnessError> {
    let modules = directory.join("node_modules");
    let asker = directory.join("asker");
    for path in [&modules, &asker] {
        fs::create_dir(path)?;
        set_directory_permissions(path)?;
    }
    // The same package scope the workspace writes, for the same reason: the
    // climb has to end inside this directory rather than at an ancestor that
    // could answer one of these specifiers itself.
    write_private_file(
        &asker.join(PRIVATE_PACKAGE_SCOPE_MANIFEST_NAME),
        PRIVATE_PACKAGE_SCOPE_MANIFEST,
    )?;
    for condition in candidates {
        let name = format!("{CONDITION_PACKAGE_PREFIX}{condition}");
        let package = modules.join(&name);
        fs::create_dir(&package)?;
        write_private_file(
            &package.join("package.json"),
            format!(
                "{{\"name\":\"{name}\",\"exports\":{{\".\":{{\"{condition}\":\
                 \"./applied.mjs\",\"default\":\"./unapplied.mjs\"}}}}}}\n"
            )
            .as_bytes(),
        )?;
        write_private_file(
            &package.join("applied.mjs"),
            b"export const applied = true;\n",
        )?;
        write_private_file(
            &package.join("unapplied.mjs"),
            b"export const applied = false;\n",
        )?;
    }
    let script = asker.join("ask-conditions.mjs");
    write_private_file(&script, CONDITION_OBSERVER.as_bytes())?;

    let mut command = Command::new(node_executable);
    for condition in requested {
        command.arg(format!("--conditions={condition}"));
    }
    command.arg(&script);
    for condition in candidates {
        command.arg(condition);
    }
    command
        .current_dir(directory)
        .env_clear()
        .env("HOME", directory)
        .env("TMPDIR", directory)
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("NODE_OPTIONS", "")
        .stdin(Stdio::null());
    let output = command.output().map_err(|error| {
        ProbeHarnessError::NodeProvenance(format!(
            "could not ask the pinned Node executable which export conditions it applies: {error}"
        ))
    })?;
    if !output.status.success() {
        return Err(ProbeHarnessError::NodeProvenance(format!(
            "the pinned Node executable did not report its export conditions: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let parse = |prefix: &str| -> Result<Vec<String>, ProbeHarnessError> {
        let line = text
            .lines()
            .find_map(|line| line.strip_prefix(prefix))
            .ok_or_else(|| {
                ProbeHarnessError::NodeProvenance(format!(
                    "the pinned Node executable reported no {prefix} export-condition line"
                ))
            })?;
        let mut applied = Vec::new();
        for condition in line.split(',').filter(|value| !value.is_empty()) {
            // Only what it was asked about: an answer naming anything else is
            // not an answer to this question.
            if !candidates.contains(&condition) {
                return Err(ProbeHarnessError::NodeProvenance(format!(
                    "the pinned Node executable reported export condition {condition:?}, which it \
                     was not asked about"
                )));
            }
            applied.push(condition.to_owned());
        }
        applied.sort();
        applied.dedup();
        Ok(applied)
    };
    Ok((parse("esm:")?, parse("require:")?))
}

/// Refuses when the pinned interpreter would not select the artifact case's own
/// runtime target for `kind`.
///
/// This is the planning-time half of the condition binding, and it is a
/// *reproducibility* check rather than a subset test. The artifact case was
/// selected under the requested conditions plus `default`; the interpreter
/// applies its own set on top of the flags it is given, and `module-sync`,
/// `node`, `node-addons`, and the kind's own `import`/`require` are in it. So
/// Rust replays its own selection under the set the interpreter reported and
/// requires the same file. A `require` condition requested for an ESM recipe
/// refuses here whenever the package's `exports` would answer an `import` key
/// first, which is the case the reviewer's table names.
///
/// It does not subsume [`verify_reported_resolution`]: this replays *Rust's*
/// resolver under the interpreter's condition set, and the run frame's echo is
/// what proves the interpreter itself landed there.
fn refuse_unreproducible_artifact_case(
    plan: &CertificationPlan,
    kind: ProbeImportKind,
    observed: &ObservedConditions,
) -> Result<(), ProbeHarnessError> {
    let manifest_bytes = plan.snapshot.read("package.json").ok_or_else(|| {
        ProbeHarnessError::ConditionMismatch(
            "the artifact snapshot carries no package manifest to replay resolution against".into(),
        )
    })?;
    let manifest: super::SnapshotPackageManifest =
        serde_json::from_slice(manifest_bytes).map_err(|error| {
            ProbeHarnessError::ConditionMismatch(format!(
                "the artifact snapshot's package manifest cannot drive resolution: {error}"
            ))
        })?;
    let conditions = observed
        .for_kind(kind)
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let entrypoint = plan.resolved_import.requested_entrypoint.as_str();
    let expected = plan.verified_resolution.runtime_path();
    let applied = || observed.for_kind(kind).join(",");
    let replayed = super::resolve_snapshot_export(
        &plan.snapshot,
        &manifest,
        entrypoint,
        &conditions,
        super::ResolutionAxis::Runtime,
    )
    .map_err(|error| {
        ProbeHarnessError::ConditionMismatch(format!(
            "the pinned interpreter applies [{}] for a {} import, and {entrypoint:?} does not \
             resolve under that set: {error}",
            applied(),
            kind.as_str()
        ))
    })?;
    if replayed.path != expected {
        return Err(ProbeHarnessError::ConditionMismatch(format!(
            "the pinned interpreter applies [{}] for a {} import, which selects {:?} for \
             {entrypoint:?}, but this transaction certifies {expected:?}: the probe would observe \
             a different file than the Type Facts witness read",
            applied(),
            kind.as_str(),
            replayed.path
        )));
    }
    Ok(())
}

/// The verifier-computed policy digest for the Stage 1 process scheme.
///
/// It names exactly what is done, including what is *not* done, so a receipt
/// can never be read as claiming OS-level denial or network isolation.
///
/// The `resolution:` fields mirror, one for one, the module-resolution
/// disposition table in `docs/adr/0006-probe-harness-binding.md`: every step of
/// Node's ESM and CommonJS resolvers, the input it consults, and whether this
/// scheme contains it, watches it, refuses on it, or does not deny it. Three
/// review rounds each found a different escape from the private workspace by
/// walking one step of that algorithm, so the table is the enumeration and
/// these fields are its receipt-visible form. A step whose disposition changes
/// must change a field here and bump `scheme-version`.
pub(crate) fn sandbox_policy_digest() -> Digest {
    static DIGEST: OnceLock<Digest> = OnceLock::new();
    DIGEST
        .get_or_init(|| root("probe-sandbox-policy", SANDBOX_POLICY_FIELDS))
        .clone()
}

/// The field vector [`sandbox_policy_digest`] commits to, in this exact order.
///
/// `tests::the_sandbox_policy_digest_names_what_is_not_denied` compares it
/// against a literal copy, so dropping a field, renaming one, or bumping the
/// scheme version without saying what changed fails a test rather than
/// silently re-labelling every receipt's policy binding.
pub(crate) const SANDBOX_POLICY_FIELDS: [&str; 46] = [
    "scheme-version:10",
    "profile:inert-or-import-free-or-relative-ts-graph-esm,explicit-controlled-consumer,ordinary-acceptance-refused",
    "transform:pinned-node-strip-only,parser-runtime-token-preservation,all-derived-outputs-compared,watched-derived-graph",
    "resolution:profile-hook-exact-source-url-and-authenticated-relative-edge-map,unmapped-profile-imports-refused",
    "enforcement:detect-and-refuse",
    "private-directory-mode:0700",
    "snapshot:private-copy-per-transaction",
    // Version 6: the workspace carries the analyzed package *and* the
    // dependency closure the transaction authenticated, so a recipe can import
    // the package under test and the package can resolve its own dependencies.
    // The three fields below say where those bytes may come from, that a name
    // with two authenticated versions refuses rather than being chosen between,
    // and that a dependency the package imports and nothing authenticated
    // refuses by name.
    "snapshot:analyzed-package-plus-authenticated-dependency-closure",
    "snapshot:dependency-closure-from-transaction-authenticated-snapshots-only",
    "snapshot:one-version-per-dependency-name-or-refuse",
    "snapshot:unauthenticated-package-dependency-refuses-by-name",
    "snapshot:dependency-materialization-manifest-bound-to-probe-root",
    "cwd:private-directory",
    "environment:allowlisted-not-inherited",
    "argv:worker-path-plus-requested-conditions-only",
    // One field per resolution step the table dispositions.
    "resolution:private-package-scope",
    "resolution:no-package-self-reference",
    "resolution:no-package-imports-escape",
    "resolution:no-ancestor-node-modules",
    "resolution:no-cjs-global-folders",
    "resolution:no-node-path",
    "resolution:no-environment-loader-hooks",
    "resolution:file-format-from-private-package-scope",
    "resolution:requested-conditions-passed-as-interpreter-flags",
    "resolution:conditions-observed-from-pinned-interpreter",
    "resolution:declared-import-kind-per-recipe",
    "resolution:declared-dependency-specifiers-per-recipe",
    "resolution:artifact-case-runtime-target-reproduced-or-refused",
    "report:dedicated-descriptor-3,exactly-one-run-frame",
    "report:resolution-echoed-and-compared-to-artifact-case",
    "report:declared-dependency-resolutions-echoed-and-required-inside-the-authenticated-copy",
    "startup-frame:protocol+nonce+node-version+platform+architecture",
    "process-group:own-group-killed-on-every-exit",
    "watched:private-directory-entries,private-node-modules,private-dependency-copies,harness-image,recipe-modules,private-package-scopes,home-node-modules,home-node-libraries,node-prefix-lib-node,ancestor-node-modules,ancestor-package-json,node-executable,type-facts-image,verifier-image",
    "watched-when:before-first-launch,between-launches,every-exit-path",
    "watched-node-executable:reasserted-against-build-pin",
    "network:not-denied",
    "filesystem-writes:not-denied",
    "filesystem-reads:not-denied",
    "absolute-and-file-url-imports:not-denied",
    "data-url-imports:not-denied",
    "builtin-node-modules:not-denied",
    "in-realm-loader-hooks:not-denied",
    "in-realm-intrinsic-mutation:frozen-prototypes-only",
    "child-processes:not-denied",
    "native-addons-and-inherited-descriptors:not-denied",
];

/// The environment identity every session records, and the worker has to echo.
///
/// `conditions` is tagged rather than bare, because two different facts belong
/// on the record and conflating them is what made the old constant
/// `["import", "node"]` a lie: `requested:` is the set this plan's artifact
/// case was *selected* under, and `esm:`/`require:` are the sets the pinned
/// interpreter reported that it actually *applies* for each import kind. The
/// two differ — `module-sync` and `node-addons` are in the second and not the
/// first — and the difference decides which file a bare specifier lands on.
fn probe_environment(
    node_executable_sha256: &str,
    node_version: &str,
    requested: &[String],
    observed: &ObservedConditions,
) -> EnvironmentIdentity {
    let mut conditions = Vec::new();
    for condition in requested {
        conditions.push(format!("requested:{condition}"));
    }
    for condition in &observed.esm {
        conditions.push(format!("esm:{condition}"));
    }
    for condition in &observed.require {
        conditions.push(format!("require:{condition}"));
    }
    EnvironmentIdentity {
        runtime: ToolIdentity {
            name: "node".into(),
            version: node_version.into(),
            build: Digest::parse(node_executable_sha256)
                .expect("the Node executable digest was validated"),
            protocol: Some(PROBE_WORKER_PROTOCOL.into()),
        },
        os: std::env::consts::OS.into(),
        architecture: std::env::consts::ARCH.into(),
        conditions,
        sandbox: SandboxIdentity {
            kind: SandboxKind::Process,
            policy: Some(sandbox_policy_digest()),
        },
    }
}

fn verify_node_executable(path: &Path, pin: &ProbeHarnessPin) -> Result<String, ProbeHarnessError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ProbeHarnessError::NodeProvenance(format!(
            "could not inspect the pinned Node executable: {error}"
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(ProbeHarnessError::NodeProvenance(
            "the pinned Node executable must be named by its real path, not a symlink".into(),
        ));
    }
    if !metadata.file_type().is_file() {
        return Err(ProbeHarnessError::NodeProvenance(
            "the pinned Node executable must be a regular file".into(),
        ));
    }
    let observed = hash_file(path).map_err(|error| {
        ProbeHarnessError::NodeProvenance(format!("could not hash the Node executable: {error}"))
    })?;
    if observed != pin.node_executable_sha256 {
        return Err(ProbeHarnessError::NodeProvenance(format!(
            "the Node executable bytes do not match this build's configured digest \
             (configured {}, observed {observed})",
            pin.node_executable_sha256
        )));
    }
    Ok(observed)
}

/// The harness image bytes this transaction verified, kept so the private
/// directory is populated from *them* and never from a second read of the same
/// paths.
///
/// Re-reading after verifying would leave a window in which the file on disk
/// can be replaced between the hash and the copy, and the copy is what the
/// worker then executes.
pub(crate) struct VerifiedHarnessImage {
    manifest_sha256: String,
    members: BTreeMap<&'static str, Vec<u8>>,
}

impl VerifiedHarnessImage {
    /// The verified bytes of one manifest member.
    fn member(&self, name: &'static str) -> Result<&[u8], ProbeHarnessError> {
        self.members.get(name).map(Vec::as_slice).ok_or_else(|| {
            ProbeHarnessError::HarnessProvenance(format!(
                "harness image member {name} was not part of the verified manifest"
            ))
        })
    }
}

/// Names the image by identity rather than by content: the members are whole
/// script bodies, and a diagnostic that dumps them says less, not more.
impl std::fmt::Debug for VerifiedHarnessImage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerifiedHarnessImage")
            .field("manifest_sha256", &self.manifest_sha256)
            .field("members", &self.members.len())
            .finish()
    }
}

fn verify_harness_image(
    harness_root: &Path,
    pin: &ProbeHarnessPin,
) -> Result<VerifiedHarnessImage, ProbeHarnessError> {
    let image = harness_image(harness_root)?;
    if image.manifest_sha256 != pin.harness_manifest_sha256 {
        return Err(ProbeHarnessError::HarnessProvenance(format!(
            "the harness image does not match this build's configured source manifest \
             (configured {}, observed {})",
            pin.harness_manifest_sha256, image.manifest_sha256
        )));
    }
    verify_harness_stamp(harness_root, pin)?;
    Ok(image)
}

/// Recomputes the harness source manifest from the bytes on disk, in exactly
/// the framing `scripts/probe-harness-source-identity.mjs` uses, and keeps the
/// bytes it hashed.
pub(crate) fn harness_image(
    harness_root: &Path,
) -> Result<VerifiedHarnessImage, ProbeHarnessError> {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker-probe-harness-source\0");
    hash.update(b"1");
    hash.update(b"\0");
    let mut names = HARNESS_MANIFEST_FILES;
    names.sort_unstable();
    let mut members = BTreeMap::new();
    for name in names {
        let path = harness_root.join(name);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            ProbeHarnessError::HarnessProvenance(format!(
                "could not inspect harness image member {name}: {error}"
            ))
        })?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(ProbeHarnessError::HarnessProvenance(format!(
                "harness image member {name} must be a regular non-symlink file"
            )));
        }
        let contents = fs::read(&path).map_err(|error| {
            ProbeHarnessError::HarnessProvenance(format!(
                "could not read harness image member {name}: {error}"
            ))
        })?;
        hash.update(name.as_bytes());
        hash.update(b"\0");
        hash.update(contents.len().to_string().as_bytes());
        hash.update(b"\0");
        hash.update(&contents);
        hash.update(b"\0");
        members.insert(name, contents);
    }
    Ok(VerifiedHarnessImage {
        manifest_sha256: format!("sha256:{:x}", hash.finalize()),
        members,
    })
}

/// The manifest digest of the image at `harness_root`, for the tracer tests
/// that assemble or inspect an image without launching one. Production always
/// keeps the bytes it hashed, so it never needs the digest on its own.
#[cfg(test)]
pub(crate) fn harness_source_manifest(harness_root: &Path) -> Result<String, ProbeHarnessError> {
    Ok(harness_image(harness_root)?.manifest_sha256)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HarnessBuildInfo {
    format: u32,
    source_digest: String,
}

/// Cross-checks the build-provenance stamp beside the CLI. The stamp is never
/// the root: it is compared *to* the compiled-in manifest digest, so a stamp
/// that agrees with a tampered image is still a refusal, because the recomputed
/// manifest already failed.
fn verify_harness_stamp(
    harness_root: &Path,
    pin: &ProbeHarnessPin,
) -> Result<(), ProbeHarnessError> {
    let path = harness_root.join(HARNESS_STAMP);
    let bytes = fs::read(&path).map_err(|error| {
        ProbeHarnessError::HarnessProvenance(format!(
            "could not read the probe harness source-manifest stamp {HARNESS_STAMP}: {error}"
        ))
    })?;
    let info: HarnessBuildInfo = serde_json::from_slice(&bytes).map_err(|error| {
        ProbeHarnessError::HarnessProvenance(format!(
            "invalid probe harness source-manifest stamp: {error}"
        ))
    })?;
    if info.format != 1 || format!("sha256:{}", info.source_digest) != pin.harness_manifest_sha256 {
        return Err(ProbeHarnessError::HarnessProvenance(
            "the probe harness source-manifest stamp does not match the configured pin".into(),
        ));
    }
    Ok(())
}

/// Asks the pinned Node bytes for their own version. The answer is as
/// trustworthy as the bytes, which were hashed against the compiled-in pin
/// first; the worker then has to echo the same string or be killed.
fn node_version(path: &Path, node_sha256: &str) -> Result<String, ProbeHarnessError> {
    static CACHE: OnceLock<std::sync::Mutex<BTreeMap<String, String>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(BTreeMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some(version) = guard.get(node_sha256)
    {
        return Ok(version.clone());
    }
    let output = Command::new(path)
        .arg("--version")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .map_err(|error| {
            ProbeHarnessError::NodeProvenance(format!(
                "could not ask the pinned Node executable for its version: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(ProbeHarnessError::NodeProvenance(
            "the pinned Node executable did not report a version".into(),
        ));
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if version.is_empty() || version.len() > 256 {
        return Err(ProbeHarnessError::NodeProvenance(
            "the pinned Node executable reported an unusable version string".into(),
        ));
    }
    if let Ok(mut guard) = cache.lock() {
        guard.insert(node_sha256.to_owned(), version.clone());
    }
    Ok(version)
}

// ---------------------------------------------------------------------------
// Recipe corpus
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRecipeCorpus {
    format: String,
    schema_version: u16,
    policy: WireRecipePolicy,
    recipes: Vec<WireRecipeEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRecipePolicy {
    repeat_runs: u16,
    timeout_millis: u64,
    max_microtask_turns: u16,
    max_macrotask_turns: u16,
    max_events: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRecipeEntry {
    claim_id: String,
    module: String,
    /// How this recipe reaches the package. Required, and not defaulted: the
    /// two kinds resolve under different condition sets, so a corpus that does
    /// not say which one it uses is not saying what the gate observed.
    import_kind: ProbeImportKind,
    /// Bare specifiers of the analyzed package's dependencies this recipe needs
    /// resolvable. Optional and defaulted to none, so a corpus that declares
    /// nothing behaves exactly as before.
    ///
    /// Declaring one asks for two things and gets both: the specifier must name
    /// a dependency whose snapshot this transaction authenticated (else the
    /// gate refuses by name before a launch), and every launch's echoed
    /// resolution for it must land inside that authenticated private copy.
    #[serde(default)]
    dependency_specifiers: Vec<String>,
    scenario: WireRecipeScenario,
    expected_event: WireRecipeEvent,
    drain: Vec<WireRecipeDrainStep>,
    #[serde(default)]
    coverage_limitations: Vec<String>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireRecipeScenario {
    Operation,
    CleanupLifecycle,
    RepeatedAsyncIterable,
    TransitionLifecycle,
    RequestResponseLifecycle,
    RootLifetime,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRecipeEvent {
    marker: String,
    class: ProbeEventClass,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum WireRecipeDrainStep {
    Flush,
    Microtasks { max_turns: u16 },
    Macrotasks { max_turns: u16 },
}

/// One hand-authored, claim-addressed recipe module plus the bytes Rust read
/// for it. `construction` is always Rust's own digest of those bytes.
#[derive(Debug)]
pub(crate) struct CorpusRecipe {
    claim_id: String,
    file_name: String,
    bytes: Vec<u8>,
    construction: Digest,
    import_kind: ProbeImportKind,
    dependency_specifiers: Vec<String>,
    scenario: ProbeScenario,
    expected_event: ProbeEventMatch,
    drain: Vec<DrainStep>,
    coverage_limitations: Vec<String>,
}

/// The recipe corpus for one certification transaction.
///
/// The corpus is an *input*, not a root of trust: a corpus that omits a
/// scheduled gate makes that gate refuse (`MissingGate`), and a vacuous recipe
/// only fails to veto — it can never establish closure, which remains the Type
/// Facts `DomainExhaustiveness` witness's job. Corpus provenance is Stage 3.
pub(crate) struct RecipeCorpus {
    policy: ProbePolicy,
    recipes: Vec<CorpusRecipe>,
    root: Digest,
}

impl RecipeCorpus {
    pub(crate) fn load(
        directory: &Path,
        plan: &CertificationPlan,
    ) -> Result<Self, ProbeHarnessError> {
        if !directory.is_dir() {
            return Err(ProbeHarnessError::CorpusInvalid(
                "the probe recipe corpus path is not a directory".into(),
            ));
        }
        // A package must never supply the probe that vetoes its own proposed
        // closure, so the corpus may not live inside the analyzed package.
        let package_root = plan.import_request.importer.as_str();
        if !package_root.is_empty()
            && let Ok(canonical) = fs::canonicalize(directory)
            && let Ok(importer) = fs::canonicalize(Path::new(package_root))
        {
            let importer_directory = if importer.is_dir() {
                importer.clone()
            } else {
                importer.parent().map(Path::to_path_buf).unwrap_or(importer)
            };
            if canonical.starts_with(&importer_directory) {
                return Err(ProbeHarnessError::CorpusInvalid(
                    "the probe recipe corpus may not live inside the analyzed package".into(),
                ));
            }
        }
        let manifest_path = directory.join(RECIPE_CORPUS_MANIFEST);
        let bytes = read_bounded_regular_file(&manifest_path, MAX_CORPUS_BYTES)
            .map_err(|error| ProbeHarnessError::CorpusInvalid(error.to_string()))?;
        let manifest: WireRecipeCorpus = serde_json::from_slice(&bytes).map_err(|error| {
            ProbeHarnessError::CorpusInvalid(format!("invalid probe recipe corpus: {error}"))
        })?;
        if manifest.format != RECIPE_CORPUS_FORMAT
            || manifest.schema_version != RECIPE_CORPUS_SCHEMA_VERSION
        {
            return Err(ProbeHarnessError::CorpusInvalid(format!(
                "the probe recipe corpus must use format {RECIPE_CORPUS_FORMAT:?} \
                 schemaVersion {RECIPE_CORPUS_SCHEMA_VERSION}"
            )));
        }
        let policy = ProbePolicy {
            repeat_runs: manifest.policy.repeat_runs,
            timeout_millis: manifest.policy.timeout_millis,
            max_microtask_turns: manifest.policy.max_microtask_turns,
            max_macrotask_turns: manifest.policy.max_macrotask_turns,
            max_events: manifest.policy.max_events,
        };
        let mut recipes = Vec::with_capacity(manifest.recipes.len());
        let mut seen = BTreeSet::new();
        for entry in manifest.recipes {
            if !seen.insert(entry.claim_id.clone()) {
                return Err(ProbeHarnessError::CorpusInvalid(format!(
                    "the probe recipe corpus repeats claim {}",
                    entry.claim_id
                )));
            }
            let file_name = single_relative_component(&entry.module)?;
            let module_path = directory.join(&file_name);
            let module_bytes = read_bounded_regular_file(&module_path, MAX_RECIPE_BYTES)
                .map_err(|error| ProbeHarnessError::RecipeProvenance(error.to_string()))?;
            let construction = Digest::parse(format!("sha256:{:x}", Sha256::digest(&module_bytes)))
                .expect("SHA-256 formatting is canonical");
            let mut coverage_limitations = entry.coverage_limitations;
            coverage_limitations.sort();
            coverage_limitations.dedup();
            // Sorted and deduplicated so the corpus root is a property of the
            // declared *set*, and validated as plain npm names here so a
            // specifier can never become a path component the workspace did not
            // intend.
            let mut dependency_specifiers = entry.dependency_specifiers;
            dependency_specifiers.sort();
            dependency_specifiers.dedup();
            for specifier in &dependency_specifiers {
                safe_package_directory(specifier).map_err(|error| {
                    ProbeHarnessError::CorpusInvalid(format!(
                        "probe recipe for claim {} declares dependency specifier \
                         {specifier:?}, which is not a bare package specifier: {error}",
                        entry.claim_id
                    ))
                })?;
            }
            recipes.push(CorpusRecipe {
                claim_id: entry.claim_id,
                file_name,
                bytes: module_bytes,
                construction,
                import_kind: entry.import_kind,
                dependency_specifiers,
                scenario: entry.scenario.into(),
                expected_event: ProbeEventMatch {
                    marker: entry.expected_event.marker,
                    class: entry.expected_event.class,
                    // A closure veto is a domain-level claim, so its marker
                    // never names one operation of the export.
                    operation: None,
                },
                drain: entry.drain.into_iter().map(Into::into).collect(),
                coverage_limitations,
            });
        }
        recipes.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
        let root = corpus_root(&policy, &recipes);
        Ok(Self {
            policy,
            recipes,
            root,
        })
    }

    pub(crate) const fn policy(&self) -> ProbePolicy {
        self.policy
    }

    pub(crate) fn recipe_for(&self, claim_id: &str) -> Option<&CorpusRecipe> {
        self.recipes
            .iter()
            .find(|recipe| recipe.claim_id == claim_id)
    }
}

impl CorpusRecipe {
    pub(crate) const fn construction(&self) -> &Digest {
        &self.construction
    }

    pub(crate) const fn import_kind(&self) -> ProbeImportKind {
        self.import_kind
    }

    pub(crate) fn dependency_specifiers(&self) -> &[String] {
        &self.dependency_specifiers
    }

    pub(crate) const fn scenario(&self) -> ProbeScenario {
        self.scenario
    }

    pub(crate) fn expected_event(&self) -> &ProbeEventMatch {
        &self.expected_event
    }

    pub(crate) fn drain(&self) -> &[DrainStep] {
        &self.drain
    }

    pub(crate) fn coverage_limitations(&self) -> &[String] {
        &self.coverage_limitations
    }
}

impl From<WireRecipeScenario> for ProbeScenario {
    fn from(value: WireRecipeScenario) -> Self {
        match value {
            WireRecipeScenario::Operation => Self::Operation,
            WireRecipeScenario::CleanupLifecycle => Self::CleanupLifecycle,
            WireRecipeScenario::RepeatedAsyncIterable => Self::RepeatedAsyncIterable,
            WireRecipeScenario::TransitionLifecycle => Self::TransitionLifecycle,
            WireRecipeScenario::RequestResponseLifecycle => Self::RequestResponseLifecycle,
            WireRecipeScenario::RootLifetime => Self::RootLifetime,
        }
    }
}

impl From<WireRecipeDrainStep> for DrainStep {
    fn from(value: WireRecipeDrainStep) -> Self {
        match value {
            WireRecipeDrainStep::Flush => Self::Flush,
            WireRecipeDrainStep::Microtasks { max_turns } => Self::Microtasks { max_turns },
            WireRecipeDrainStep::Macrotasks { max_turns } => Self::Macrotasks { max_turns },
        }
    }
}

fn corpus_root(policy: &ProbePolicy, recipes: &[CorpusRecipe]) -> Digest {
    let mut values = vec![
        format!("repeat-runs:{}", policy.repeat_runs),
        format!("timeout-millis:{}", policy.timeout_millis),
        format!("max-microtask-turns:{}", policy.max_microtask_turns),
        format!("max-macrotask-turns:{}", policy.max_macrotask_turns),
        format!("max-events:{}", policy.max_events),
    ];
    for recipe in recipes {
        values.push(format!(
            "recipe:{}:{}:{}:{}",
            recipe.claim_id,
            recipe.file_name,
            recipe.import_kind.as_str(),
            recipe.construction.as_str()
        ));
        // Appended per declared specifier rather than folded into the line
        // above, so a corpus that declares none produces a byte-identical
        // root: the receipt binding of every recipe written before this field
        // existed is unchanged.
        for specifier in &recipe.dependency_specifiers {
            values.push(format!("recipe-dependency:{}:{specifier}", recipe.claim_id));
        }
    }
    root("probe-recipe-corpus", values.iter().map(String::as_str))
}

// ---------------------------------------------------------------------------
// Private workspace and launch
// ---------------------------------------------------------------------------

/// One certification transaction's private 0700 execution directory.
///
/// Layout:
///
/// ```text
/// <private>/
///   node_modules/<package-name>/…   private copy of the artifact snapshot
///   node_modules/<dependency>/…     one private copy per authenticated
///                                   dependency snapshot, `@scope/` kept
///   harness/contract-probe-worker.mjs, contract-probe-harness.mjs
///   recipes/<file>.mjs              copied recipe modules
/// ```
///
/// A dependency copy carries its *own* authenticated `package.json` — this
/// module writes none there — because that manifest's `exports` map is what
/// answers its specifiers. `LOOKUP_PACKAGE_SCOPE` from a file inside the copy
/// finds it and then returns null at the `node_modules` segment above, so the
/// climb ends inside the copy and cannot reach the two private scopes or any
/// ancestor's manifest.
///
/// `harness/` and `recipes/` additionally hold a `package.json` this module
/// writes, with no `exports`, `imports`, or `main`. It is the containment for
/// `PACKAGE_SELF_RESOLVE` and `PACKAGE_IMPORTS_RESOLVE`; see
/// [`create_private_layout`].
///
/// A bare specifier the private copy answers resolves there. One it does *not*
/// answer is not contained by the layout alone: Node walks `node_modules`
/// upwards from the importer, so `<tmpdir>/node_modules` and `/node_modules`
/// are both candidates; `HOME` pointing at the private directory makes
/// `<private>/.node_modules` and `<private>/.node_libraries` CommonJS global
/// folders; and `$PREFIX/lib/node`, derived from the Node executable's own
/// install prefix, is the third one. Containment is therefore two explicit
/// properties rather than a consequence of the layout:
/// [`refuse_resolvable_bare_specifier_sources`] refuses a private directory
/// with any of those above or beside it *before* the first launch, and every
/// candidate then joins the watched census — together with the two `HOME`
/// folders and every ancestor `package.json` the private scopes shadow — so one
/// appearing during a run refuses the gate. The precondition alone would not be
/// enough: the workspace lives under a shared, on Linux world-writable
/// temporary directory, so `<ancestor>/node_modules` can be created after the
/// check.
struct PrivateProbeWorkspace {
    directory: PathBuf,
    worker: PathBuf,
    recipes: BTreeMap<String, (PathBuf, String)>,
    /// The runtime target the artifact case names, inside the private copy.
    ///
    /// Every launch's reported module resolution has to name this exact file:
    /// it is the one the Type Facts witness read for the selected export
    /// conditions, and a probe that observed any other file observed a
    /// different artifact case than the one being certified.
    runtime_target: PathBuf,
    execution: Option<runtime_probe_wire::InertExecutionRequest>,
    /// Where each authenticated dependency copy sits, by package name.
    ///
    /// A recipe that declares it needs a dependency specifier has its echoed
    /// resolution for that specifier required to name a file *inside* the
    /// matching root ([`verify_reported_resolution`]), which is how "the
    /// dependency import reached authenticated bytes" becomes a checked
    /// property of the launch rather than a property of the layout.
    dependency_roots: BTreeMap<String, PathBuf>,
    /// One `--conditions=<name>` flag per requested export condition, in the
    /// order a launch passes them.
    condition_flags: Vec<String>,
    watched: Vec<(String, WatchedInput)>,
    /// Watched labels whose digest is known *a priori*, checked on the baseline
    /// census and on every re-census.
    ///
    /// `node-executable` is the only one, and it needs this because a census
    /// that merely records what was on disk when the baseline was taken cannot
    /// notice bytes swapped between [`verify_node_executable`] and that
    /// baseline: it would faithfully record — and then faithfully confirm — the
    /// substituted binary. The pin says what those bytes must be, so the census
    /// asserts the pin instead of asserting self-consistency.
    pinned: Vec<(String, String)>,
    /// The baseline census, keyed by label.
    ///
    /// A map rather than a list, because both readers look a label *up*: a
    /// duplicate label in a list would make [`Self::verify_pinned`]'s
    /// first-match silently authoritative over its twin, and
    /// [`Self::verify_unchanged`]'s positional zip would compare two censuses
    /// that agree in length while disagreeing about which path each entry is.
    /// [`Self::watch_digests`] refuses a duplicate outright instead.
    before: BTreeMap<String, String>,
}

/// Everything one private workspace is built from, in one place.
///
/// A parameter list rather than a struct grew past what Clippy's
/// `too_many_arguments` accepts once the dependency closure joined it, and the
/// struct is the better shape anyway: every field is an authenticated input,
/// and naming them at the call site says which is which.
struct PrivateWorkspaceInputs<'a> {
    plan: &'a CertificationPlan,
    image: &'a VerifiedHarnessImage,
    corpus: &'a RecipeCorpus,
    node_executable: &'a Path,
    /// The *pin's* value for the Node bytes, so the watched census re-asserts
    /// it rather than recording whatever was on disk when the baseline ran.
    node_executable_sha256: &'a str,
    type_facts_pin: &'a TypeFactsProducerPin,
    requested_conditions: &'a [String],
    dependencies: &'a BTreeMap<String, AuthenticatedDependency<'a>>,
}

impl PrivateProbeWorkspace {
    fn create(inputs: &PrivateWorkspaceInputs<'_>) -> Result<Self, ProbeHarnessError> {
        Self::create_with_inert(inputs, None)
    }

    fn create_with_inert(
        inputs: &PrivateWorkspaceInputs<'_>,
        inert: Option<(&super::controlled_execution::InertModule, bool)>,
    ) -> Result<Self, ProbeHarnessError> {
        let PrivateWorkspaceInputs {
            plan,
            image,
            corpus,
            node_executable,
            node_executable_sha256,
            type_facts_pin,
            requested_conditions,
            dependencies,
        } = *inputs;
        let directory = create_private_directory("harness")?;
        // Refuse before anything is copied: every location outside the private
        // tree from which a *bare* specifier the private copy does not answer
        // could be resolved — each `<ancestor>/node_modules`, and the CommonJS
        // `$PREFIX/lib/node` global folder derived from the Node executable's
        // own install prefix. This transaction authenticated none of those
        // bytes. The same candidates are watched below, because this check is a
        // point-in-time one on a shared tree.
        refuse_resolvable_bare_specifier_sources(&directory, node_executable)?;
        let PrivateLayout {
            modules: modules_directory,
            harness: harness_directory,
            recipes: recipes_directory,
        } = create_private_layout(&directory)?;

        for member in HARNESS_MODULES {
            let name = Path::new(member)
                .file_name()
                .expect("harness module paths name a file");
            // The bytes verify_harness_image hashed, not a second read of the
            // same path: re-reading would be a window for a swap.
            write_private_file(&harness_directory.join(name), image.member(member)?)?;
        }

        let mut recipes = BTreeMap::new();
        for recipe in &corpus.recipes {
            let target = recipes_directory.join(&recipe.file_name);
            write_private_file(&target, &recipe.bytes)?;
            recipes.insert(
                recipe.claim_id.clone(),
                (target, format!("recipes/{}", recipe.file_name)),
            );
        }

        // The probe reads a private copy of the snapshot, never the shared
        // materialized store and never the analyzed tree. The package name is
        // a resolved-import field, so it is validated before it is joined:
        // `validate_coordinate` upstream admits a `..` segment.
        let package_directory =
            modules_directory.join(safe_package_directory(&plan.resolved_import.package_name)?);
        copy_snapshot_into(&package_directory, &plan.snapshot)?;

        // The authenticated dependency closure, beside the analyzed package and
        // under exactly the same discipline: authenticated snapshot bytes, a
        // validated npm name as its directory, and — below — its own watched
        // census entry. This is what lets a recipe `import` the package under
        // test at all, because the package's own top-level
        // `import "<dependency>"` runs in the worker as soon as it does.
        //
        // Each dependency's own `package.json` is the authenticated one and is
        // never replaced: it carries the `exports` map its specifiers resolve
        // through. `LOOKUP_PACKAGE_SCOPE` from a file inside the copy finds
        // that manifest first and then returns null at the `node_modules`
        // segment above it, so the climb cannot leave `<private>/node_modules`
        // and never reaches the two private scopes or an ancestor's.
        let mut dependency_roots = BTreeMap::new();
        for (name, dependency) in dependencies {
            let target = modules_directory.join(&dependency.directory);
            copy_snapshot_into(&target, dependency.snapshot)?;
            dependency_roots.insert(name.clone(), target);
        }

        // The exact file the Type Facts witness read for the selected export
        // conditions, as it sits in the private copy.
        let runtime_target = package_directory.join(single_safe_relative_path(
            plan.verified_resolution.runtime_path(),
        )?);
        if !runtime_target.is_file() {
            return Err(ProbeHarnessError::Configuration(format!(
                "the artifact case's runtime target {:?} is not part of the private snapshot copy",
                plan.verified_resolution.runtime_path()
            )));
        }

        let worker = harness_directory.join(WORKER_ENTRY);
        let execution = if let Some((module, consumer)) = inert {
            let mut execution_modules = Vec::new();
            let mut private_sources = BTreeMap::new();
            let mut root_derived = None;
            for (index, derived_module) in module.modules.iter().enumerate() {
                let source =
                    package_directory.join(single_safe_relative_path(&derived_module.source_path)?);
                if !source.is_file() {
                    return Err(ProbeHarnessError::Configuration(format!(
                        "controlled graph source {:?} is not part of the private snapshot copy",
                        derived_module.source_path
                    )));
                }
                let source = fs::canonicalize(source)?;
                let derived = harness_directory.join(format!("controlled-derived-{index}.mjs"));
                write_private_file(&derived, derived_module.output.as_bytes())?;
                let derived = fs::canonicalize(derived)?;
                if derived_module.source_path == module.source_path {
                    root_derived = Some(derived.clone());
                }
                private_sources.insert(derived_module.source_path.clone(), source.clone());
                execution_modules.push(runtime_probe_wire::DerivedExecutionModuleRequest {
                    source_path: source.to_string_lossy().into_owned(),
                    derived_path: derived.to_string_lossy().into_owned(),
                    source_digest: derived_module.source_digest.clone(),
                    output_digest: derived_module.output_digest.clone(),
                });
            }
            let (derived, modules, edges) = if module.modules.is_empty() {
                let derived = harness_directory.join("inert-derived.mjs");
                write_private_file(&derived, module.output.as_bytes())?;
                (fs::canonicalize(derived)?, Vec::new(), Vec::new())
            } else {
                let edges = module
                    .edges
                    .iter()
                    .map(|edge| {
                        Ok(runtime_probe_wire::DerivedExecutionEdgeRequest {
                            importer_path: private_sources
                                .get(&edge.importer_path)
                                .ok_or_else(|| {
                                    ProbeHarnessError::Configuration(format!(
                                        "controlled graph importer {:?} has no derived module",
                                        edge.importer_path
                                    ))
                                })?
                                .to_string_lossy()
                                .into_owned(),
                            specifier: edge.specifier.clone(),
                            target_path: private_sources
                                .get(&edge.target_path)
                                .ok_or_else(|| {
                                    ProbeHarnessError::Configuration(format!(
                                        "controlled graph target {:?} has no derived module",
                                        edge.target_path
                                    ))
                                })?
                                .to_string_lossy()
                                .into_owned(),
                        })
                    })
                    .collect::<Result<Vec<_>, ProbeHarnessError>>()?;
                (
                    root_derived.expect("a controlled graph always contains its root"),
                    execution_modules,
                    edges,
                )
            };
            Some(runtime_probe_wire::InertExecutionRequest {
                profile: module.profile.into(),
                // Node canonicalizes the enclosing /var -> /private/var alias
                // on macOS. Bind that exact URL, without changing the selected
                // source or weakening the independent resolution check.
                source_path: fs::canonicalize(&runtime_target)?
                    .to_string_lossy()
                    .into_owned(),
                derived_path: fs::canonicalize(&derived)?.to_string_lossy().into_owned(),
                source_digest: module.source_digest.clone(),
                output_digest: module.output_digest.clone(),
                export_name: module.export_name.clone(),
                consumer,
                modules,
                edges,
            })
        } else {
            None
        };
        let mut watched = vec![
            // The private directory itself, by direct entry rather than by
            // content: `TMPDIR` and the cwd are this directory, so a
            // `node.config.json` or any other file landing beside the three
            // subdirectories has to be a change even though its content is not
            // watched. The subdirectories' contents are watched in full below.
            (
                "private-directory-entries".to_owned(),
                WatchedInput::DirectoryEntries(directory.clone()),
            ),
            // The whole private `node_modules`, not just the package copy: a
            // run that adds a sibling package would otherwise be unwatched.
            (
                "private-node-modules".to_owned(),
                WatchedInput::Contents(modules_directory),
            ),
            (
                "harness-image".to_owned(),
                WatchedInput::Contents(harness_directory),
            ),
            (
                "recipe-modules".to_owned(),
                WatchedInput::Contents(recipes_directory),
            ),
            // `HOME` is the private directory, so these two are CommonJS
            // global folders for the worker. Neither exists; either appearing
            // is a violation.
            (
                "home-node-modules".to_owned(),
                WatchedInput::Contents(directory.join(".node_modules")),
            ),
            (
                "home-node-libraries".to_owned(),
                WatchedInput::Contents(directory.join(".node_libraries")),
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
        // One entry per dependency copy, on top of the whole-tree
        // `private-node-modules` census that already covers them. The
        // redundancy is deliberate and cheap: a dependency file changing
        // mid-run then refuses by *name* rather than as an anonymous change
        // somewhere under `node_modules`, and a future change that stopped
        // placing a dependency inside the watched tree would fail here instead
        // of leaving it unwatched. Labels cannot collide: the closure is keyed
        // by package name, and two versions of one name were refused above.
        watched.extend(dependency_roots.iter().map(|(name, root)| {
            (
                format!("private-dependency:{name}"),
                WatchedInput::Contents(root.clone()),
            )
        }));
        // Every location outside the private tree a bare specifier could reach.
        // `refuse_resolvable_bare_specifier_sources` proved each absent a
        // moment ago, so each is recorded absent and one appearing mid-run is a
        // violation.
        watched.extend(
            bare_specifier_sources(&directory, node_executable)
                .into_iter()
                .map(|(label, path)| (label, WatchedInput::Contents(path))),
        );
        // Every `package.json` above the private tree. The two private scopes
        // written by `create_private_layout` terminate `LOOKUP_PACKAGE_SCOPE`
        // before any of these is consulted, so this census is the belt to that
        // brace: it catches one being planted mid-run, and it fails loudly if a
        // future change ever removes a private scope.
        watched.extend(
            ancestor_package_scopes(&directory)
                .into_iter()
                .map(|(label, path)| (label, WatchedInput::Contents(path))),
        );
        if let Ok(verifier) = std::env::current_exe() {
            watched.push((
                "verifier-image".to_owned(),
                WatchedInput::Contents(verifier),
            ));
        }
        let mut workspace = Self {
            directory,
            worker,
            recipes,
            runtime_target,
            execution,
            dependency_roots,
            condition_flags: requested_conditions
                .iter()
                .map(|condition| format!("--conditions={condition}"))
                .collect(),
            watched,
            pinned: vec![(
                "node-executable".to_owned(),
                node_executable_sha256.to_owned(),
            )],
            before: BTreeMap::new(),
        };
        // Re-read every copy *after* the permission changes and before the
        // first launch, exactly as the Type Facts image is re-hashed.
        workspace.before = workspace.watch_digests()?;
        workspace.verify_pinned(&workspace.before)?;
        Ok(workspace)
    }

    fn recipe(&self, claim_id: &str) -> Option<(&Path, &str)> {
        self.recipes
            .get(claim_id)
            .map(|(path, relative)| (path.as_path(), relative.as_str()))
    }

    /// One census of every watched path, keyed by label.
    ///
    /// A repeated label is refused rather than merged: the whole census is
    /// consulted by label, so two entries under one name would make one of them
    /// unreadable and the other silently authoritative.
    fn watch_digests(&self) -> Result<BTreeMap<String, String>, ProbeHarnessError> {
        let mut census = BTreeMap::new();
        for (label, path) in &self.watched {
            let digest = watch_digest(path)?;
            if census.insert(label.clone(), digest).is_some() {
                return Err(ProbeHarnessError::IsolationViolation(format!(
                    "the watched probe input census names {label} twice, so one entry would be \
                     unreadable"
                )));
            }
        }
        Ok(census)
    }

    /// Re-asserts every watched path whose digest this build knows in advance.
    ///
    /// Equality across a run only proves self-consistency: two censuses of a
    /// binary that was already substituted agree with each other. For the one
    /// watched path with an a-priori expected value — the Node executable the
    /// pin names — the census compares against the *pin*, so a swap between
    /// [`verify_node_executable`] and any later read refuses the gate.
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

    /// Refuses the gate when any watched input changed across the run, or when
    /// a watched path with a pinned digest is not those bytes. This is
    /// detection, not denial: it proves the transaction's other reads were not
    /// disturbed by a probe, and never claims the probe *could not* write.
    fn verify_unchanged(&self) -> Result<(), ProbeHarnessError> {
        let after = self.watch_digests()?;
        self.verify_pinned(&after)?;
        for (label, before) in &self.before {
            match after.get(label) {
                Some(after) if after == before => {}
                Some(_) => {
                    return Err(ProbeHarnessError::IsolationViolation(format!(
                        "{label} changed while the runtime probe was executing"
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

    /// Launches one isolated worker process and returns its single run.
    ///
    /// Rust owns the launch: the executable is the pinned Node path, the
    /// environment is allowlisted rather than inherited, the working directory
    /// is inside the private tree, the frames arrive on a descriptor package
    /// code cannot reach by name, and the startup frame must echo the protocol,
    /// this launch's nonce, and the version, platform, and architecture Rust
    /// established before the session is written.
    ///
    /// Every exit path kills the worker's whole process group. That is what
    /// bounds the transaction: without it a detached grandchild inheriting the
    /// report descriptor keeps the pipe open, and a reader waiting for EOF
    /// waits forever. No reader is joined either — a thread that may never
    /// finish is dropped, not waited on.
    fn launch(
        &self,
        node_executable: &Path,
        node_version: &str,
        module: &Path,
        session_bytes: &[u8],
        timeout_millis: u64,
        subject: ResolutionSubject<'_>,
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
        let mut command = Command::new(node_executable);
        command
            // The requested export conditions, so the interpreter applies the
            // set this plan's artifact case was selected under where it can.
            // Not a substitute for the resolution check below: `module-sync`
            // and `require` still win over a requested `import` in a package
            // that lists them first.
            .args(&self.condition_flags)
            .arg(&self.worker)
            .current_dir(&self.directory)
            .stdin(Stdio::piped())
            // Nothing is read from stdout, so nothing package code writes
            // there can be mistaken for a frame.
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        command.env_clear();
        // Only what Node needs to start plus the recipe path and this launch's
        // nonce. `PATH` is deliberately absent: the worker is launched by
        // absolute path and must not be able to find anything by name.
        command.env("HOME", &self.directory);
        command.env("TMPDIR", &self.directory);
        command.env("LANG", "C");
        command.env("LC_ALL", "C");
        command.env("NODE_OPTIONS", "");
        command.env("SOLID_CHECKER_PROBE_RECIPE", module);
        command.env("SOLID_CHECKER_PROBE_NONCE", &nonce);

        let (child, report) = spawn_reporting_worker(&mut command)?;
        let mut worker = WorkerProcess::new(child);
        let process_id = worker.process_id();
        let stderr = worker.take_stderr();
        let diagnostics = spawn_stderr_reader(stderr);
        let receiver = spawn_report_reader(report);

        let deadline = Instant::now() + Duration::from_millis(timeout_millis.max(1));
        let startup = receive_line(
            &receiver,
            STARTUP_BUDGET.min(deadline.saturating_duration_since(Instant::now())),
        )
        .map_err(|error| worker.explain(error, &diagnostics))?;
        verify_startup_frame(&startup, &nonce, node_version)?;

        if let Some(mut stdin) = worker.take_stdin() {
            let mut session: serde_json::Value = serde_json::from_slice(session_bytes)
                .map_err(|error| ProbeHarnessError::Protocol(error.to_string()))?;
            if let Some(execution) = &self.execution {
                session["execution"] = serde_json::to_value(execution)
                    .map_err(|error| ProbeHarnessError::Protocol(error.to_string()))?;
            }
            let session_bytes = serde_json::to_vec(&session)
                .map_err(|error| ProbeHarnessError::Protocol(error.to_string()))?;
            let written = stdin
                .write_all(&session_bytes)
                .and_then(|()| stdin.write_all(b"\n"))
                .and_then(|()| stdin.flush());
            drop(stdin);
            written.map_err(|error| {
                worker.explain(
                    ProbeHarnessError::Launch(format!(
                        "could not write the probe session to the worker: {error}"
                    )),
                    &diagnostics,
                )
            })?;
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        let run_line = receive_line(&receiver, remaining)
            .map_err(|error| worker.explain(error, &diagnostics))?;
        // Nothing more is wanted from the worker, so its whole group dies here
        // rather than at the end of the function: an extra frame can then only
        // be one it had already written.
        drop(worker);
        match receiver.recv_timeout(EXTRA_FRAME_GRACE) {
            Ok(Ok(extra)) if !extra.trim().is_empty() => {
                return Err(ProbeHarnessError::Protocol(
                    "the probe worker wrote more than one run frame; exactly one is the protocol"
                        .into(),
                ));
            }
            Ok(Err(error)) => {
                return Err(ProbeHarnessError::Protocol(format!(
                    "the probe worker's report descriptor is not protocol-conforming: {error}"
                )));
            }
            Ok(Ok(_)) | Err(_) => {}
        }

        let decoded = runtime_probe_wire::decode_probe_run(run_line.as_bytes())?;
        // The worker's isolation identity is transport data: bind it to the
        // process Rust actually launched instead of believing it.
        if !decoded
            .run
            .isolation
            .process
            .starts_with(&format!("{process_id}:"))
        {
            return Err(ProbeHarnessError::Protocol(format!(
                "the probe worker reported process identity {:?}, which does not name the \
                 launched process {process_id}",
                decoded.run.isolation.process
            )));
        }
        verify_reported_resolution(
            decoded.resolution.as_ref(),
            subject,
            &self.runtime_target,
            &self.dependency_roots,
        )?;
        match (&self.execution, &decoded.execution) {
            (None, None) => {}
            (Some(expected), Some(actual))
                if actual.binding == *expected
                    && actual.loaded
                    && actual.consumer_completed == expected.consumer => {}
            _ => {
                return Err(ProbeHarnessError::Protocol(format!(
                    "execution profile/input/output/consumer echo mismatch or source was not loaded (stage: {})",
                    decoded
                        .execution
                        .as_ref()
                        .map_or("missing", |actual| actual.stage.as_str())
                )));
            }
        }
        Ok(decoded.run)
    }
}

/// One launched worker, plus the guarantee that its whole process group is
/// killed however this function leaves.
///
/// `Child::kill` reaches the direct child only. A worker that forked and
/// detached leaves a grandchild holding the report descriptor open, and every
/// bounded wait in this module then depends on a process nobody is tracking.
/// Each launch therefore gets its own process group, and dropping this kills
/// the group.
struct WorkerProcess {
    child: Child,
    group: i32,
}

impl WorkerProcess {
    fn new(child: Child) -> Self {
        // `process_group(0)` makes the child a group leader, so its pid is the
        // group id.
        let group = i32::try_from(child.id()).unwrap_or(-1);
        Self { child, group }
    }

    fn process_id(&self) -> u32 {
        self.child.id()
    }

    fn take_stdin(&mut self) -> Option<std::process::ChildStdin> {
        self.child.stdin.take()
    }

    fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    /// Kills the group, then attaches whatever the worker managed to say on
    /// stderr to a launch failure. Nothing semantic is read from it.
    fn explain(
        &mut self,
        error: ProbeHarnessError,
        diagnostics: &mpsc::Receiver<String>,
    ) -> ProbeHarnessError {
        self.kill_group();
        let tail = diagnostics
            .recv_timeout(EXTRA_FRAME_GRACE)
            .unwrap_or_default();
        match error {
            ProbeHarnessError::Launch(message) if !tail.is_empty() => {
                ProbeHarnessError::Launch(format!("{message} (worker stderr: {tail})"))
            }
            other => other,
        }
    }

    fn kill_group(&mut self) {
        #[cfg(unix)]
        if self.group > 0 {
            // SAFETY: `killpg` takes a process-group id and a signal number.
            unsafe {
                libc::killpg(self.group, libc::SIGKILL);
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for WorkerProcess {
    fn drop(&mut self) {
        self.kill_group();
    }
}

/// Spawns the worker with its own process group and a pipe on
/// [`REPORT_DESCRIPTOR`], returning the read end.
///
/// The descriptor is the point: the worker reports on a channel it was handed,
/// so package top-level code replacing `process.stdout.write` reaches a stream
/// nothing reads.
#[cfg(unix)]
fn spawn_reporting_worker(command: &mut Command) -> Result<(Child, File), ProbeHarnessError> {
    use std::os::{
        fd::{AsRawFd as _, FromRawFd as _, OwnedFd},
        unix::process::CommandExt as _,
    };

    let mut descriptors = [0_i32; 2];
    // SAFETY: `pipe` fills the two-element array it is given.
    if unsafe { libc::pipe(descriptors.as_mut_ptr()) } != 0 {
        return Err(ProbeHarnessError::Launch(format!(
            "could not create the probe report descriptor: {}",
            std::io::Error::last_os_error()
        )));
    }
    // SAFETY: both descriptors were just created by `pipe` and are owned here.
    let read = unsafe { OwnedFd::from_raw_fd(descriptors[0]) };
    // SAFETY: as above.
    let write = unsafe { OwnedFd::from_raw_fd(descriptors[1]) };
    let read_raw = read.as_raw_fd();
    let write_raw = write.as_raw_fd();
    // Close-on-exec on both ends, immediately. `pipe2(O_CLOEXEC)` would be the
    // one-call form, but macOS has no `pipe2`, so the flag is set here — still
    // before any other thread can spawn. Without it any concurrent
    // `Command::spawn` in this process (another launch, the Type Facts
    // producer) inherits the write end and holds the pipe open, and the reader
    // below then waits for an EOF that never comes. The child's own end is
    // exempted in `pre_exec`: `dup2` clears the flag on the descriptor it
    // creates.
    for descriptor in [read_raw, write_raw] {
        set_close_on_exec(descriptor).map_err(|error| {
            ProbeHarnessError::Launch(format!(
                "could not make the probe report descriptor close-on-exec: {error}"
            ))
        })?;
    }

    command.process_group(0);
    // SAFETY: the closure runs between fork and exec in the child and calls
    // only async-signal-safe functions.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(write_raw, REPORT_DESCRIPTOR) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if write_raw == REPORT_DESCRIPTOR {
                // `dup2` onto itself is a no-op, so the descriptor kept the
                // close-on-exec flag set above and the worker would exec with
                // no report channel. Clear it for this one descriptor only.
                let flags = libc::fcntl(REPORT_DESCRIPTOR, libc::F_GETFD);
                if flags == -1
                    || libc::fcntl(REPORT_DESCRIPTOR, libc::F_SETFD, flags & !libc::FD_CLOEXEC)
                        == -1
                {
                    return Err(std::io::Error::last_os_error());
                }
            } else {
                libc::close(write_raw);
            }
            if read_raw != REPORT_DESCRIPTOR {
                libc::close(read_raw);
            }
            Ok(())
        });
    }
    let child = command.spawn().map_err(|error| {
        ProbeHarnessError::Launch(format!("could not launch the probe worker: {error}"))
    })?;
    // The parent must not keep the write end: the reader below would never see
    // EOF if it did.
    drop(write);
    Ok((child, File::from(read)))
}

/// Sets `FD_CLOEXEC` on one descriptor.
#[cfg(unix)]
fn set_close_on_exec(descriptor: i32) -> std::io::Result<()> {
    // SAFETY: `F_GETFD`/`F_SETFD` take a descriptor and an int flag set.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags == -1 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: as above.
    if unsafe { libc::fcntl(descriptor, libc::F_SETFD, flags | libc::FD_CLOEXEC) } == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
fn spawn_reporting_worker(_command: &mut Command) -> Result<(Child, File), ProbeHarnessError> {
    Err(ProbeHarnessError::Configuration(
        "this platform cannot give the probe worker a private report descriptor".into(),
    ))
}

/// Reads the report descriptor on a detached thread, bounded in both frames and
/// bytes.
///
/// The thread is never joined: a grandchild the group kill missed could hold
/// the pipe open indefinitely, and a joined reader would make that the
/// transaction's problem. The channel is sized for the frames the protocol
/// allows plus the one refusal the reader may send, so the reader never blocks
/// on it either.
fn spawn_report_reader(report: File) -> mpsc::Receiver<std::io::Result<String>> {
    let (sender, receiver) = mpsc::sync_channel::<std::io::Result<String>>(MAX_REPORT_FRAMES + 1);
    std::thread::spawn(move || {
        let mut reader = BufReader::new(report);
        let mut remaining = MAX_REPORT_BYTES;
        let mut frames = 0_usize;
        loop {
            let mut line = String::new();
            // One byte past the budget is enough to notice the budget was
            // exceeded without buffering what follows it.
            let budget = u64::try_from(remaining)
                .unwrap_or(u64::MAX)
                .saturating_add(1);
            match reader.by_ref().take(budget).read_line(&mut line) {
                // End of stream after the allowed frames is the ordinary case;
                // the budget is only exceeded by a frame that actually arrives.
                Ok(0) => break,
                Ok(read) if read > remaining => {
                    let _ = sender.send(Err(std::io::Error::other(format!(
                        "the report exceeded its {MAX_REPORT_BYTES}-byte budget"
                    ))));
                    break;
                }
                Ok(read) => {
                    if line.trim().is_empty() {
                        // A trailing newline is not a frame.
                        remaining = remaining.saturating_sub(read);
                        continue;
                    }
                    if frames >= MAX_REPORT_FRAMES {
                        let _ = sender.send(Err(std::io::Error::other(format!(
                            "more than {MAX_REPORT_FRAMES} report frames were written"
                        ))));
                        break;
                    }
                    remaining -= read;
                    frames += 1;
                    if sender.send(Ok(line)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                    break;
                }
            }
        }
    });
    receiver
}

/// Reads the worker's stderr on a detached thread and delivers a bounded tail
/// once, at EOF. Diagnostics only — no verdict depends on it.
fn spawn_stderr_reader(stderr: Option<ChildStderr>) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::sync_channel::<String>(1);
    if let Some(stderr) = stderr {
        std::thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = stderr
                .take(MAX_STDERR_BYTES as u64)
                .read_to_end(&mut buffer);
            let _ = sender.send(String::from_utf8_lossy(&buffer).trim().to_owned());
        });
    }
    receiver
}

impl Drop for PrivateProbeWorkspace {
    fn drop(&mut self) {
        let _ = remove_private_tree(&self.directory);
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireWorkerStartup {
    format: String,
    protocol: String,
    nonce: String,
    node_version: String,
    platform: String,
    architecture: String,
}

fn verify_startup_frame(
    line: &str,
    nonce: &str,
    node_version: &str,
) -> Result<(), ProbeHarnessError> {
    let frame: WireWorkerStartup = serde_json::from_str(line.trim()).map_err(|error| {
        ProbeHarnessError::Protocol(format!("invalid probe worker startup frame: {error}"))
    })?;
    if frame.format != STARTUP_FORMAT {
        return Err(ProbeHarnessError::Protocol(format!(
            "probe worker startup frame must use format {STARTUP_FORMAT:?}"
        )));
    }
    if frame.protocol != PROBE_WORKER_PROTOCOL {
        return Err(ProbeHarnessError::Protocol(format!(
            "probe worker speaks protocol {:?}; this verifier speaks {PROBE_WORKER_PROTOCOL:?}",
            frame.protocol
        )));
    }
    if frame.nonce != nonce {
        return Err(ProbeHarnessError::Protocol(
            "probe worker did not echo this launch's nonce".into(),
        ));
    }
    if frame.node_version != node_version {
        return Err(ProbeHarnessError::Protocol(format!(
            "probe worker runs Node {:?}; the pinned executable reported {node_version:?}",
            frame.node_version
        )));
    }
    // The recorded environment's `os` and `architecture` are the *verifier's*
    // `std::env::consts`, so the worker has to confirm it is running on the
    // same platform rather than have that assumed of it — the same reason its
    // Node version is checked instead of taken on trust.
    let expected_platform = node_platform_name();
    if frame.platform != expected_platform {
        return Err(ProbeHarnessError::Protocol(format!(
            "probe worker reports platform {:?}; this verifier records {expected_platform:?} \
             ({})",
            frame.platform,
            std::env::consts::OS
        )));
    }
    let expected_architecture = node_architecture_name();
    if frame.architecture != expected_architecture {
        return Err(ProbeHarnessError::Protocol(format!(
            "probe worker reports architecture {:?}; this verifier records \
             {expected_architecture:?} ({})",
            frame.architecture,
            std::env::consts::ARCH
        )));
    }
    Ok(())
}

/// Requires the resolution the worker reported to name the artifact case's own
/// runtime target.
///
/// This is the runtime half of the condition binding, and it is what makes a
/// wrongly-selected artifact case a refusal instead of a false pass. The
/// worker resolves the plan's own specifier *before* it imports the recipe —
/// and therefore before any package code runs — and Rust compares the answer
/// against the one file the Type Facts witness read.
///
/// Three things it does not claim:
///
/// * The worker resolves from its own module URL, which sits in
///   `<private>/harness/` rather than in `<private>/recipes/`. The two
///   directories carry byte-identical package scopes and share the same
///   `node_modules` ancestry, so they can only disagree if a `node_modules`
///   appears inside one of them — and both trees are watched whole, so that is
///   an [`ProbeHarnessError::IsolationViolation`] on the next census.
/// * A missing report is a refusal, not an absence: every certification launch
///   is given a resolution subject, so a run frame without one is a worker that
///   did not answer.
/// * Nothing here defends against in-realm resolver patching *after* the
///   package is imported. It does not need to: the recipe's own import is
///   resolved before the package evaluates, and each launch runs one recipe.
///
/// # The declared dependency specifiers
///
/// A recipe that declares dependency specifiers has each one checked the same
/// way, with one weaker claim stated rather than glossed: the answer proves the
/// specifier resolves **inside that dependency's authenticated private copy**,
/// not that it resolves to one exact file, because a dependency's own `exports`
/// map may legitimately answer several entrypoints and this transaction
/// certifies no artifact case for it.
///
/// It is also resolved from the worker's URL (ESM) or the recipe's (CommonJS),
/// not from inside the analyzed package's copy, so it is evidence about the
/// rung that answers rather than about the package's own walk. That rung is the
/// same one: `<private>/harness`, `<private>/recipes`, and
/// `<private>/node_modules/<package>` all reach `<private>/node_modules` and
/// nothing above it — every ancestor candidate is refused as a precondition and
/// watched afterwards, and both importer directories are watched whole, so a
/// nearer `node_modules` appearing inside one is an
/// [`ProbeHarnessError::IsolationViolation`]. A nested `node_modules` the
/// *snapshot itself* ships would shadow the copy, and that is not an escape:
/// those bytes are authenticated too, being part of the snapshot.
fn verify_reported_resolution(
    reported: Option<&ReportedResolution>,
    subject: ResolutionSubject<'_>,
    expected: &Path,
    dependency_roots: &BTreeMap<String, PathBuf>,
) -> Result<(), ProbeHarnessError> {
    let ResolutionSubject {
        specifier,
        import_kind,
        dependencies,
    } = subject;
    let Some(reported) = reported else {
        return Err(ProbeHarnessError::ConditionMismatch(
            "the probe worker reported no module resolution, so nothing proves it observed the \
             artifact case this transaction certifies"
                .into(),
        ));
    };
    if reported.specifier != specifier || reported.import_kind != import_kind.as_str() {
        return Err(ProbeHarnessError::ConditionMismatch(format!(
            "the probe worker reported a resolution of {:?} as {:?}; this launch asked for \
             {specifier:?} as {:?}",
            reported.specifier,
            reported.import_kind,
            import_kind.as_str()
        )));
    }
    let (observed, resolved) = match import_kind {
        ProbeImportKind::Esm => (reported.esm.as_str(), decode_file_url(&reported.esm)),
        ProbeImportKind::Require => {
            let path = Path::new(&reported.require);
            (
                reported.require.as_str(),
                path.is_absolute().then(|| path.to_path_buf()),
            )
        }
    };
    let Some(resolved) = resolved else {
        return Err(ProbeHarnessError::ConditionMismatch(format!(
            "the probe worker resolved {specifier:?} as {:?} to {observed:?}, which is not a \
             local file this verifier can compare against {expected:?}",
            import_kind.as_str()
        )));
    };
    if !names_same_file(&resolved, expected) {
        return Err(ProbeHarnessError::ConditionMismatch(format!(
            "the probe worker resolved {specifier:?} as {:?} to {observed:?}, but the artifact \
             case this transaction certifies names {expected:?}: the probe ran against a \
             different file than the Type Facts witness read",
            import_kind.as_str()
        )));
    }
    verify_reported_dependency_resolutions(reported, import_kind, dependencies, dependency_roots)
}

/// Requires one echoed resolution per declared dependency specifier, in the
/// order asked, each landing inside that dependency's authenticated copy.
///
/// The count and the order are Rust's, not the worker's: a frame carrying fewer
/// entries, more entries, or the same entries permuted is a refusal rather than
/// a set to search, so a worker cannot answer a cheap specifier twice and leave
/// the interesting one unproven.
fn verify_reported_dependency_resolutions(
    reported: &ReportedResolution,
    import_kind: ProbeImportKind,
    dependencies: &[String],
    dependency_roots: &BTreeMap<String, PathBuf>,
) -> Result<(), ProbeHarnessError> {
    if reported.dependencies.len() != dependencies.len() {
        return Err(ProbeHarnessError::ConditionMismatch(format!(
            "this launch asked the probe worker to resolve {} declared dependency specifier(s) \
             and it reported {}",
            dependencies.len(),
            reported.dependencies.len()
        )));
    }
    for (specifier, answer) in dependencies.iter().zip(&reported.dependencies) {
        if &answer.specifier != specifier {
            return Err(ProbeHarnessError::ConditionMismatch(format!(
                "the probe worker reported a dependency resolution of {:?} where this launch \
                 asked for {specifier:?}",
                answer.specifier
            )));
        }
        let root = dependency_roots.get(specifier).ok_or_else(|| {
            ProbeHarnessError::ConditionMismatch(format!(
                "the probe recipe declared dependency {specifier:?}, which this workspace placed \
                 no authenticated copy for"
            ))
        })?;
        let (observed, resolved) = match import_kind {
            ProbeImportKind::Esm => (answer.esm.as_str(), decode_file_url(&answer.esm)),
            ProbeImportKind::Require => {
                let path = Path::new(&answer.require);
                (
                    answer.require.as_str(),
                    path.is_absolute().then(|| path.to_path_buf()),
                )
            }
        };
        let Some(resolved) = resolved else {
            return Err(ProbeHarnessError::ConditionMismatch(format!(
                "the probe worker resolved dependency {specifier:?} as {:?} to {observed:?}, \
                 which is not a local file this verifier can compare against the authenticated \
                 copy at {root:?}",
                import_kind.as_str()
            )));
        };
        if !path_is_inside(&resolved, root) {
            return Err(ProbeHarnessError::ConditionMismatch(format!(
                "the probe worker resolved dependency {specifier:?} as {:?} to {observed:?}, \
                 which is not inside the authenticated private copy at {root:?}: the probe would \
                 have run against bytes this transaction never authenticated",
                import_kind.as_str()
            )));
        }
    }
    Ok(())
}

/// Whether `candidate` names a file under `root`.
///
/// Both sides are canonicalized for the same reason [`names_same_file`] does
/// it: on macOS `TMPDIR` lives under `/var`, a symlink to `/private/var`, and
/// Node realpaths what it resolves. A path that cannot be canonicalized is not
/// treated as inside, which is the fail-closed direction.
fn path_is_inside(candidate: &Path, root: &Path) -> bool {
    match (fs::canonicalize(candidate), fs::canonicalize(root)) {
        (Ok(candidate), Ok(root)) => candidate.starts_with(&root),
        _ => false,
    }
}

/// One `file:` URL as the local path it names, or `None` when it names anything
/// else.
///
/// A URL with an authority, a non-hexadecimal escape, or bytes that are not
/// UTF-8 is not a local path this verifier can compare, and saying so is the
/// fail-closed direction. The query and fragment are dropped: the worker
/// appends neither, and a resolver that did would be naming the same file.
fn decode_file_url(value: &str) -> Option<PathBuf> {
    let rest = value.strip_prefix("file://")?;
    if !rest.starts_with('/') {
        return None;
    }
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'?' | b'#' => break,
            b'%' => {
                let high = char::from(*bytes.get(index + 1)?).to_digit(16)?;
                let low = char::from(*bytes.get(index + 2)?).to_digit(16)?;
                decoded.push(u8::try_from(high * 16 + low).ok()?);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    Some(PathBuf::from(String::from_utf8(decoded).ok()?))
}

/// Whether two paths name one file.
///
/// The literal comparison is not enough on macOS, where `TMPDIR` is under
/// `/var` — a symlink to `/private/var` — and Node realpaths what it resolves.
/// Canonicalizing both is what makes "the same file, spelled differently" the
/// same file; a path that cannot be canonicalized is not treated as equal.
fn names_same_file(candidate: &Path, expected: &Path) -> bool {
    if candidate == expected {
        return true;
    }
    match (fs::canonicalize(candidate), fs::canonicalize(expected)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

/// Node's `process.platform` for the platform this verifier was built for.
///
/// An unmapped `std::env::consts::OS` falls through to its own name, which
/// matches only if Node happens to spell it the same way and refuses otherwise.
/// That is the fail-closed direction: a platform nobody mapped refuses rather
/// than skipping the check.
fn node_platform_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

/// Node's `process.arch` for the architecture this verifier was built for, with
/// the same fail-closed fallback as [`node_platform_name`].
fn node_architecture_name() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "ia32",
        "aarch64" => "arm64",
        "powerpc64" => "ppc64",
        "loongarch64" => "loong64",
        other => other,
    }
}

fn receive_line(
    receiver: &mpsc::Receiver<std::io::Result<String>>,
    budget: Duration,
) -> Result<String, ProbeHarnessError> {
    match receiver.recv_timeout(budget) {
        Ok(Ok(line)) => Ok(line),
        Ok(Err(error)) => Err(ProbeHarnessError::Launch(format!(
            "could not read from the probe worker: {error}"
        ))),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(ProbeHarnessError::Timeout),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(ProbeHarnessError::Launch(
            "the probe worker exited without answering".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Filesystem helpers
// ---------------------------------------------------------------------------

fn create_private_directory(purpose: &str) -> Result<PathBuf, ProbeHarnessError> {
    let base = std::env::temp_dir();
    for _ in 0..128 {
        let candidate = base.join(format!(
            "solid-checker-probe-{purpose}-{}-{}",
            std::process::id(),
            PRIVATE_HARNESS_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => {
                set_directory_permissions(&candidate)?;
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(ProbeHarnessError::Launch(
        "could not allocate a private probe execution directory".into(),
    ))
}

#[cfg(unix)]
fn set_directory_permissions(path: &Path) -> Result<(), ProbeHarnessError> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_directory_permissions(_path: &Path) -> Result<(), ProbeHarnessError> {
    Err(ProbeHarnessError::Configuration(
        "this platform cannot establish private probe execution permissions".into(),
    ))
}

#[cfg(unix)]
fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), ProbeHarnessError> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private_file(_path: &Path, _bytes: &[u8]) -> Result<(), ProbeHarnessError> {
    Err(ProbeHarnessError::Configuration(
        "this platform cannot establish private probe execution permissions".into(),
    ))
}

fn remove_private_tree(directory: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fn relax(path: &Path) -> std::io::Result<()> {
            let metadata = fs::symlink_metadata(path)?;
            if metadata.is_dir() {
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
                for entry in fs::read_dir(path)? {
                    relax(&entry?.path())?;
                }
            } else {
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
            }
            Ok(())
        }
        let _ = relax(directory);
    }
    fs::remove_dir_all(directory)
}

fn hash_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}

/// What one watched census entry hashes.
#[derive(Clone, Debug)]
enum WatchedInput {
    /// Every regular file under a directory, or one regular file's bytes.
    Contents(PathBuf),
    /// A directory's direct entry names and kinds, without their contents.
    ///
    /// The private directory itself is watched this way. It is `TMPDIR` and
    /// the cwd, so the loader-affecting files that would be read *from* it —
    /// `node.config.json` above all — have to be noticed appearing, while the
    /// three subdirectories' contents are watched in full by their own
    /// entries. The cost is a refusal direction: a probe that writes a
    /// temporary file into `TMPDIR` refuses the gate.
    DirectoryEntries(PathBuf),
}

impl WatchedInput {
    fn path(&self) -> &Path {
        match self {
            Self::Contents(path) | Self::DirectoryEntries(path) => path,
        }
    }
}

/// One watched input's digest: a tree digest for a directory, the file digest
/// for a regular file, an entry-name digest for a
/// [`WatchedInput::DirectoryEntries`], and a fixed marker for a path that does
/// not exist.
///
/// The marker is what makes "this path appeared during the run" a change rather
/// than an error: a `HOME`-relative CommonJS global folder is absent when the
/// census is taken and must stay absent.
fn watch_digest(input: &WatchedInput) -> Result<String, ProbeHarnessError> {
    let path = input.path();
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ABSENT_WATCHED_PATH.to_owned())
        }
        Err(error) => Err(error.into()),
        Ok(metadata) if metadata.is_dir() => match input {
            WatchedInput::Contents(_) => hash_tree(path),
            WatchedInput::DirectoryEntries(_) => hash_directory_entries(path),
        },
        Ok(metadata) if metadata.file_type().is_file() => {
            hash_file(path).map_err(ProbeHarnessError::from)
        }
        Ok(_) => Err(ProbeHarnessError::IsolationViolation(format!(
            "watched probe input {} is neither a regular file nor a directory",
            path.display()
        ))),
    }
}

/// A digest over one directory's direct entries — each name and its kind — and
/// nothing below them.
fn hash_directory_entries(directory: &Path) -> Result<String, ProbeHarnessError> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        let kind = if metadata.is_dir() {
            "directory"
        } else if metadata.file_type().is_file() {
            "file"
        } else {
            "other"
        };
        entries.push((
            entry.file_name().to_string_lossy().into_owned(),
            kind.to_owned(),
        ));
    }
    entries.sort();
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:probe-private-directory-entries:v1\0");
    for (name, kind) in entries {
        hash.update(u64::try_from(name.len()).unwrap_or(u64::MAX).to_be_bytes());
        hash.update(name.as_bytes());
        hash.update(kind.as_bytes());
        hash.update(b"\0");
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}

// ---------------------------------------------------------------------------
// The authenticated dependency closure
// ---------------------------------------------------------------------------

/// One dependency of the analyzed package that this transaction authenticated,
/// as it will sit in the private workspace.
///
/// The bytes are a *snapshot* the certification transaction already
/// authenticated — an integrity-verified published archive whose lock selection
/// replayed the same name, version, and integrity — which is the same channel
/// the Type Facts private project materializes from
/// (`type_facts::snapshot_source_roots`). Nothing here reads the project's real
/// `node_modules`, a registry, or any path outside that authenticated set.
struct AuthenticatedDependency<'a> {
    /// The one or two ordinary path components the copy occupies under
    /// `<private>/node_modules`, from [`safe_package_directory`]. A scoped name
    /// keeps its `@scope/` component, because `node_modules/@scope/name` is
    /// the directory a bare specifier resolves to and `node_modules/scope/name`
    /// is a different one.
    directory: PathBuf,
    snapshot: &'a super::ArtifactSnapshot,
}

/// Every source or transitive graph dependency snapshot this transaction
/// authenticated, keyed by package name. Graph receipt composition runs before
/// this input is forwarded; no caller-supplied path becomes a snapshot here.
///
/// **One version per name, or refuse.** Two authenticated snapshots of one name
/// at different versions cannot both be `<private>/node_modules/<name>`, and the
/// alternative — placing the importer-specific copy at
/// `<private>/node_modules/<importer>/node_modules/<name>` — would require this
/// module to decide *which* importer each copy belongs to. The authenticated
/// source set does carry an `installed_package_root`, but that root describes
/// the project's real tree, not the private one, and reading it as a nesting
/// instruction would make the probe's resolution depend on a path this
/// workspace does not reproduce. So the ambiguity is refused by name. Nothing
/// in the corpus needs otherwise: every measured row's closure names each
/// dependency once, and a row that genuinely installs two versions of one
/// package refuses its gate instead of being probed against a copy chosen here.
///
/// Two entries that carry the *same* snapshot root are one copy named twice —
/// the source set is keyed by a canonical identity that includes the installed
/// root, so a package hoisted and also nested appears twice — and are placed
/// once.
fn authenticated_dependency_closure<'a>(
    plan: &'a CertificationPlan,
    graph_dependencies: &[&'a CertificationPlan],
) -> Result<BTreeMap<String, AuthenticatedDependency<'a>>, ProbeHarnessError> {
    let mut closure = BTreeMap::<String, AuthenticatedDependency<'_>>::new();
    let snapshots = plan
        .certification_sources
        .iter()
        .map(|source| &source.snapshot)
        .chain(graph_dependencies.iter().flat_map(|dependency| {
            std::iter::once(&dependency.snapshot).chain(
                dependency
                    .certification_sources
                    .iter()
                    .map(|source| &source.snapshot),
            )
        }));
    for snapshot in snapshots {
        let name = snapshot.package_name().to_owned();
        let directory = safe_package_directory(&name)?;
        if name == plan.snapshot.package_name() {
            if snapshot.root() == plan.snapshot.root() {
                continue;
            }
            return Err(ProbeHarnessError::AmbiguousDependencyVersion {
                package_name: name.clone(),
                versions: format!(
                    "{name}@{} and {name}@{} with different snapshot roots",
                    plan.snapshot.package_version(),
                    snapshot.package_version()
                ),
            });
        }
        match closure.get(&name) {
            Some(existing) if existing.snapshot.root() == snapshot.root() => continue,
            Some(existing) => {
                let mut versions = [
                    format!(
                        "{}@{}",
                        existing.snapshot.package_name(),
                        existing.snapshot.package_version()
                    ),
                    format!("{name}@{}", snapshot.package_version()),
                ];
                versions.sort();
                return Err(ProbeHarnessError::AmbiguousDependencyVersion {
                    package_name: name,
                    versions: versions.join(" and "),
                });
            }
            None => {}
        }
        closure.insert(
            name,
            AuthenticatedDependency {
                directory,
                snapshot,
            },
        );
    }
    Ok(closure)
}

fn dependency_materialization_root(
    dependencies: &BTreeMap<String, AuthenticatedDependency<'_>>,
) -> String {
    let fields = dependencies
        .iter()
        .flat_map(|(name, dependency)| [name.as_str(), dependency.snapshot.root()])
        .collect::<Vec<_>>();
    root("probe-dependency-materialization", fields)
        .as_str()
        .to_owned()
}

/// Refuses when the analyzed package imports a dependency this transaction
/// authenticated no snapshot for.
///
/// A recipe imports the package under test, so the package's own top-level
/// imports run in the worker. Probing a *partial* closure would mean one of two
/// things: the import throws (a refusal wearing the wrong name — a launch
/// failure rather than the missing-authenticated-bytes fact), or it resolves
/// somewhere this transaction never authenticated. Both are refused here, by
/// name, before the private directory exists.
///
/// The required set is the independently replayed module closure's own accepted
/// dependency edges (`verified_closure`), never a manifest's `dependencies`
/// field: the edges are what the closure replay proved this artifact case's
/// modules actually import. An `UnacceptedExternalDependency` hazard is not
/// consulted, because such a hazard already opens every affected claim domain
/// at replay and no closure candidate — and so no gate — survives it.
fn require_authenticated_dependency_closure(
    plan: &CertificationPlan,
    closure: &BTreeMap<String, AuthenticatedDependency<'_>>,
) -> Result<(), ProbeHarnessError> {
    for edge in &plan.verified_closure.manifest().dependencies {
        if closure.contains_key(&edge.package_name) {
            continue;
        }
        return Err(ProbeHarnessError::UnauthenticatedDependency(Box::new(
            UnauthenticatedDependency {
                specifier: edge.specifier.clone(),
                package_name: edge.package_name.clone(),
                importer: format!(
                    "{}@{}",
                    plan.snapshot.package_name(),
                    plan.snapshot.package_version()
                ),
            },
        )));
    }
    Ok(())
}

/// The private directory's three subdirectories.
struct PrivateLayout {
    modules: PathBuf,
    harness: PathBuf,
    recipes: PathBuf,
}

/// Creates the private directory's fixed layout, terminating module resolution
/// inside it.
///
/// The two `package.json` files are not bookkeeping. Node resolves a bare
/// specifier by trying `PACKAGE_SELF_RESOLVE` *before* the `node_modules` walk,
/// and both that step and `PACKAGE_IMPORTS_RESOLVE` (a `#specifier`) start from
/// `LOOKUP_PACKAGE_SCOPE`, which climbs from the importing module to the first
/// `package.json` it finds. Without one inside the private tree that climb
/// reaches `<tmpdir>/package.json` — a world-writable location on Linux — so a
/// planted `{"name": "<the analyzed package>", "exports": …}` there answers the
/// recipe's own bare import and the probe observes a stub instead of the
/// snapshot copy. That was a *false pass*: the gate saw a conforming value and
/// let the closure through.
///
/// A manifest with no `exports`, no `imports`, and no `main` ends the climb
/// without being able to answer anything: `PACKAGE_SELF_RESOLVE` returns
/// undefined when the nearest scope has no `exports`, the CommonJS
/// `trySelf` does the same, and a `#specifier` fails outright rather than
/// escaping. The `node_modules` walk then proceeds to the private copy.
///
/// `node_modules/` deliberately gets none: `LOOKUP_PACKAGE_SCOPE` already
/// returns null at a `node_modules` path segment, and the snapshot copy carries
/// the analyzed package's own authenticated manifest.
fn create_private_layout(directory: &Path) -> Result<PrivateLayout, ProbeHarnessError> {
    let layout = PrivateLayout {
        modules: directory.join("node_modules"),
        harness: directory.join("harness"),
        recipes: directory.join("recipes"),
    };
    for path in [&layout.modules, &layout.harness, &layout.recipes] {
        fs::create_dir(path)?;
        set_directory_permissions(path)?;
    }
    for path in [&layout.harness, &layout.recipes] {
        write_private_file(
            &path.join(PRIVATE_PACKAGE_SCOPE_MANIFEST_NAME),
            PRIVATE_PACKAGE_SCOPE_MANIFEST,
        )?;
    }
    Ok(layout)
}

/// Every location *outside* the private tree from which a bare specifier the
/// private copy does not answer could still be resolved.
///
/// Two classes, and they are enumerated in one place so that the precondition
/// below and the watched census cannot drift apart:
///
/// * `<ancestor>/node_modules` — the ESM and CommonJS `node_modules` walk from
///   an importer inside a workspace that lives under [`std::env::temp_dir`], a
///   shared directory in a shared tree. `<tmpdir>/node_modules` and
///   `/node_modules` are both candidates.
/// * `$PREFIX/lib/node` — the third CommonJS "global folder", derived from the
///   Node executable's own install prefix rather than from the environment, so
///   `env_clear` does not remove it. The other two are `HOME`-relative and
///   `HOME` is the private directory, so those are watched as part of the tree
///   this module owns.
fn bare_specifier_sources(directory: &Path, node_executable: &Path) -> Vec<(String, PathBuf)> {
    let mut sources = Vec::new();
    for ancestor in directory.ancestors().skip(1) {
        let candidate = ancestor.join("node_modules");
        sources.push((
            format!("ancestor-node-modules:{}", candidate.display()),
            candidate,
        ));
    }
    if let Some(prefix) = node_executable.parent().and_then(Path::parent) {
        let candidate = prefix.join("lib").join("node");
        sources.push((
            format!("node-prefix-lib-node:{}", candidate.display()),
            candidate,
        ));
    }
    sources
}

/// Every `package.json` above the private tree, for the watched census only.
///
/// `create_private_layout` writes a package scope inside the tree, so
/// `LOOKUP_PACKAGE_SCOPE` never reaches any of these. They are censused anyway:
/// the containment is one file per directory, and a census that records them
/// absent turns "someone planted a self-reference mid-run" and "a future change
/// dropped a private scope" into a refusal rather than a silent escape.
fn ancestor_package_scopes(directory: &Path) -> Vec<(String, PathBuf)> {
    directory
        .ancestors()
        .skip(1)
        .map(|ancestor| {
            let candidate = ancestor.join(PRIVATE_PACKAGE_SCOPE_MANIFEST_NAME);
            (
                format!("ancestor-package-json:{}", candidate.display()),
                candidate,
            )
        })
        .collect()
}

/// Refuses a private directory any bare specifier could escape from.
///
/// Refusing is the only honest answer, because the alternative is a probe
/// silently importing bytes the transaction never authenticated. The same
/// candidates are watched afterwards, because this is a point-in-time check on
/// trees this process does not own.
fn refuse_resolvable_bare_specifier_sources(
    directory: &Path,
    node_executable: &Path,
) -> Result<(), ProbeHarnessError> {
    for (_, candidate) in bare_specifier_sources(directory, node_executable) {
        match fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err(ProbeHarnessError::IsolationViolation(format!(
                    "could not establish that {} does not exist and so cannot answer a bare \
                     specifier",
                    candidate.display()
                )));
            }
            Ok(_) => {
                return Err(ProbeHarnessError::IsolationViolation(format!(
                    "{} is resolvable from the private probe directory and is not part of the \
                     authenticated snapshot copy",
                    candidate.display()
                )));
            }
        }
    }
    Ok(())
}

/// A path-ordered digest over every regular file under `directory`. A symlink
/// or any other non-regular entry appearing where a copy was written refuses,
/// so a run cannot swap a watched file for a link to elsewhere.
fn hash_tree(directory: &Path) -> Result<String, ProbeHarnessError> {
    let mut entries = Vec::new();
    collect_tree(directory, directory, &mut entries)?;
    entries.sort();
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:probe-private-tree:v1\0");
    for (relative, digest, length) in entries {
        hash.update(relative.len().to_be_bytes());
        hash.update(relative.as_bytes());
        hash.update(length.to_be_bytes());
        hash.update(digest.as_bytes());
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}

fn collect_tree(
    root_directory: &Path,
    directory: &Path,
    entries: &mut Vec<(String, String, u64)>,
) -> Result<(), ProbeHarnessError> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            collect_tree(root_directory, &path, entries)?;
        } else if metadata.file_type().is_file() {
            let relative = path
                .strip_prefix(root_directory)
                .map_err(|_| {
                    ProbeHarnessError::IsolationViolation(
                        "a watched probe input escaped its own directory".into(),
                    )
                })?
                .to_string_lossy()
                .replace('\\', "/");
            entries.push((relative, hash_file(&path)?, metadata.len()));
        } else {
            return Err(ProbeHarnessError::IsolationViolation(format!(
                "watched probe input {} is no longer a regular file",
                path.display()
            )));
        }
    }
    Ok(())
}

fn read_bounded_regular_file(path: &Path, limit: usize) -> Result<Vec<u8>, ProbeHarnessError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ProbeHarnessError::CorpusInvalid(format!("could not inspect {}: {error}", path.display()))
    })?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(ProbeHarnessError::CorpusInvalid(format!(
            "{} must be a regular non-symlink file",
            path.display()
        )));
    }
    if usize::try_from(metadata.len()).map_or(true, |length| length > limit) {
        return Err(ProbeHarnessError::CorpusInvalid(format!(
            "{} exceeds the {limit}-byte probe input limit",
            path.display()
        )));
    }
    fs::read(path).map_err(|error| {
        ProbeHarnessError::CorpusInvalid(format!("could not read {}: {error}", path.display()))
    })
}

/// One ordinary file name — no separators, no parent components, no root.
fn single_relative_component(value: &str) -> Result<String, ProbeHarnessError> {
    let path = Path::new(value);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None) => Ok(name.to_string_lossy().into_owned()),
        _ => Err(ProbeHarnessError::CorpusInvalid(format!(
            "probe recipe module {value:?} must be one plain file name inside the corpus"
        ))),
    }
}

/// One npm package name as the one or two path components it may become.
///
/// `plan.resolved_import.package_name` reaches here from a caller-supplied
/// coordinate, and the upstream `validate_coordinate` admits names a path join
/// would treat as traversal (`../..`), so the name is validated *here* before
/// it is joined onto the private `node_modules`. A scoped name becomes exactly
/// two ordinary components and an unscoped one exactly one; anything else
/// refuses.
fn safe_package_directory(name: &str) -> Result<PathBuf, ProbeHarnessError> {
    let invalid = || {
        ProbeHarnessError::Configuration(format!(
            "artifact snapshot package name {name:?} is not a plain npm package name"
        ))
    };
    // npm's own bound on a whole name, scope included.
    if name.is_empty() || name.len() > 214 {
        return Err(invalid());
    }
    // Each segment is validated bare; the scope keeps its `@` in the path,
    // because `node_modules/@scope/name` is the directory an import resolves
    // to and `node_modules/scope/name` is a different one.
    let (scope, bare) = match name.strip_prefix('@') {
        Some(scoped) => {
            let mut parts = scoped.split('/');
            let scope = parts.next().ok_or_else(invalid)?;
            let bare = parts.next().ok_or_else(invalid)?;
            if parts.next().is_some() {
                return Err(invalid());
            }
            (Some(scope), bare)
        }
        None => (None, name),
    };
    let plain = |segment: &str| {
        !segment.is_empty()
            && !segment.starts_with('.')
            // A trailing dot is not the same name everywhere: Windows strips
            // it, so `foo.` and `foo` would be one directory there and two
            // here, and the watched census would be a census of a path the
            // worker did not read.
            && !segment.ends_with('.')
            && !segment.starts_with('_')
            // Names that would make the private layout resolve somewhere else.
            // `node_modules` would nest a second one inside the first;
            // `.node_modules` and `.node_libraries` are the CommonJS global
            // folders `HOME` makes resolvable, which the census records as
            // absent. (Both dotted names are already refused above; they are
            // named here so the reason is on the record.)
            && !matches!(segment, "node_modules" | ".node_modules" | ".node_libraries")
            && segment.bytes().all(|byte| {
                matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~')
            })
    };
    let mut directory = PathBuf::new();
    if let Some(scope) = scope {
        if !plain(scope) {
            return Err(invalid());
        }
        directory.push(format!("@{scope}"));
    }
    if !plain(bare) {
        return Err(invalid());
    }
    directory.push(bare);
    // Belt and braces: the join above must have produced ordinary components
    // only, so the same rule the snapshot members are held to applies here.
    if directory
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid());
    }
    Ok(directory)
}

/// Writes one authenticated snapshot into `package_directory`, member by
/// member, with every member path held to [`single_safe_relative_path`].
///
/// Shared by the analyzed package's copy and by every dependency copy on
/// purpose: one definition of "how authenticated bytes enter the private
/// workspace" means a dependency cannot be placed under weaker rules than the
/// package under test.
fn copy_snapshot_into(
    package_directory: &Path,
    snapshot: &super::ArtifactSnapshot,
) -> Result<(), ProbeHarnessError> {
    fs::create_dir_all(package_directory)?;
    for (relative, bytes) in snapshot.files() {
        let target = package_directory.join(single_safe_relative_path(relative)?);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        write_private_file(&target, bytes)?;
    }
    Ok(())
}

/// A snapshot member path, rejected unless every component is ordinary.
fn single_safe_relative_path(value: &str) -> Result<PathBuf, ProbeHarnessError> {
    let path = Path::new(value);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ProbeHarnessError::Configuration(format!(
            "artifact snapshot member {value:?} is not a plain relative path"
        )));
    }
    Ok(path.to_path_buf())
}

fn root<'a>(domain: &'a str, values: impl IntoIterator<Item = &'a str>) -> Digest {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:probe-harness-root:v1");
    for value in std::iter::once(domain).chain(values) {
        hash.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
        hash.update(value.as_bytes());
    }
    Digest::parse(format!("sha256:{:x}", hash.finalize())).expect("SHA-256 formatting is canonical")
}

/// Builds the mode matrix and closure-falsification recipe set for one gate
/// schedule. Every input comes from the retained opaque plan, the gate ids, and
/// the recipe corpus; no caller-returned plan document is ever accepted.
pub(crate) fn runtime_probe_plan(
    plan: &CertificationPlan,
    schedule: &ProbeGateSchedule,
    corpus: &RecipeCorpus,
    environment: EnvironmentIdentity,
) -> Result<RuntimeProbePlan, ProbeHarnessError> {
    let proposal = plan.candidates.proposal();
    let mut subjects = BTreeSet::<SemanticClaimSubject>::new();
    let mut recipes = Vec::with_capacity(schedule.gates().len());
    let mut modes = Vec::new();
    let mut covered = BTreeSet::new();
    for gate in schedule.gates() {
        if !matches!(gate.subject().path, SemanticClaimPath::Domain(_)) {
            return Err(ProbeHarnessError::Configuration(
                "a probe gate must veto a claim domain".into(),
            ));
        }
        let recipe = corpus.recipe_for(gate.semantic_claim_id()).ok_or_else(|| {
            ProbeHarnessError::MissingRecipe {
                gate_id: gate.id().to_owned(),
                semantic_claim_id: gate.semantic_claim_id().to_owned(),
            }
        })?;
        subjects.insert(gate.subject().clone());
        if covered.insert(gate.subject().artifact_case.clone()) {
            modes.push(ProbeMode {
                name: "certification-probe".into(),
                artifact_case: gate.subject().artifact_case.clone(),
                environment: environment.clone(),
            });
        }
        recipes.push(ProbeRecipe {
            subject: gate.subject().clone(),
            // Rust assigns the authority: a gate is a veto, never a witness.
            authority: ProbeAuthority::ClosureFalsification,
            scenario: recipe.scenario(),
            construction: recipe.construction().clone(),
            expected_event: recipe.expected_event().clone(),
            drain: recipe.drain().to_vec(),
            coverage_limitations: recipe.coverage_limitations().to_vec(),
        });
    }
    let matrix = ArtifactModeMatrix::new(proposal, modes)?;
    Ok(RuntimeProbePlan::build(
        proposal.clone(),
        BTreeSet::new(),
        subjects,
        matrix,
        recipes,
        corpus.policy(),
    )?)
}

/// The dependency the analyzed package imports and this transaction
/// authenticated no snapshot for.
///
/// Its own type, boxed inside [`ProbeHarnessError`], so naming the specifier,
/// the package, and the importer does not enlarge every `Result` in the
/// certification lanes.
#[derive(Debug, Error)]
#[error(
    "probe workspace has no authenticated snapshot for dependency {specifier:?} (package \
     {package_name}) of {importer}: this certification transaction authenticated no bytes for it, \
     so the private workspace would carry a partial dependency closure"
)]
pub struct UnauthenticatedDependency {
    specifier: String,
    package_name: String,
    importer: String,
}

#[derive(Debug, Error)]
pub enum ProbeHarnessError {
    #[error("probe harness authority is unavailable: {0}")]
    PinUnavailable(&'static str),
    #[error("probe harness configuration is invalid: {0}")]
    Configuration(String),
    #[error("probe Node runtime provenance is invalid: {0}")]
    NodeProvenance(String),
    #[error("probe harness image provenance is invalid: {0}")]
    HarnessProvenance(String),
    #[error("probe recipe module provenance is invalid: {0}")]
    RecipeProvenance(String),
    #[error("probe recipe corpus is invalid: {0}")]
    CorpusInvalid(String),
    #[error(
        "probe gate {gate_id} has no recipe for semantic claim {semantic_claim_id} in the corpus"
    )]
    MissingRecipe {
        gate_id: String,
        semantic_claim_id: String,
    },
    /// Boxed: three `String`s inline would grow this enum — and, through
    /// `Policy2FinalizationError`, every graph-lane `Result` that wraps it —
    /// past the size Clippy's `result_large_err` accepts.
    #[error(transparent)]
    UnauthenticatedDependency(Box<UnauthenticatedDependency>),
    #[error(
        "probe workspace cannot place dependency {package_name}: this transaction authenticated \
         {versions} at distinct snapshot roots, and one private `node_modules/{package_name}` \
         cannot be both"
    )]
    AmbiguousDependencyVersion {
        package_name: String,
        versions: String,
    },
    #[error("probe write isolation was violated: {0}")]
    IsolationViolation(String),
    #[error("probe export-condition binding is invalid: {0}")]
    ConditionMismatch(String),
    #[error("probe worker launch failed: {0}")]
    Launch(String),
    #[error("probe worker protocol is invalid: {0}")]
    Protocol(String),
    #[error("the probe worker did not answer within its bounded policy timeout")]
    Timeout,
    #[error(transparent)]
    Probe(#[from] RuntimeProbeError),
    #[error(transparent)]
    Wire(#[from] RuntimeProbeWireError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests;
