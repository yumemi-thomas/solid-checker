//! Process-free Node-API/WASI entry point for browser and WebContainer hosts.

use std::path::Path;

#[cfg(feature = "reactor")]
use std::sync::Mutex;

#[cfg(feature = "napi-host")]
use napi_derive::napi;
use serde::Deserialize;
use solid_facts::TypeScriptTable;
use solid_facts_backend::SemanticDemandGroup;
use solid_facts_backend::{
    AnalysisInputs, BackendError, SemanticDemandOptions, SourceFile, TypeFactsProvider,
    analyze_project_inputs_measured_with_enablement, build_project_native_measured_with_demands,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CheckRequest {
    project_id: String,
    /// Stable id of the Solid dialect to check with; defaults to the
    /// backend's default dialect when the host names none.
    #[serde(default)]
    dialect: Option<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    type_facts: TypeScriptTable,
    /// What the host resolved each imported specifier to, in its own installed
    /// tree. Accepted and unused: only the compiled-in accepted contracts read
    /// it, and that tier is retired (ADR 0228). The authored tier is not served
    /// here: it admits on a Solid runtime environment, and this adapter, with
    /// no filesystem, can show only an empty one.
    #[serde(default)]
    #[expect(dead_code, reason = "accepted for compatibility (ADR 0228)")]
    installed_packages: Vec<HostInstalledPackage>,
    /// The export conditions the host resolved under. Accepted and unused, for
    /// the same reason.
    #[serde(default)]
    #[expect(dead_code, reason = "accepted for compatibility (ADR 0228)")]
    export_conditions: Vec<String>,
    /// Whether the contracts compiled into this build may be applied. Accepted
    /// and without effect: no compiled-in contract reaches this adapter since
    /// the certified tier retired (ADR 0228).
    #[serde(default)]
    #[expect(dead_code, reason = "accepted for compatibility (ADR 0228)")]
    bundled_contracts: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[expect(dead_code, reason = "accepted for compatibility (ADR 0228)")]
struct HostInstalledPackage {
    /// The specifier as written in the source, which is what an import states
    /// and what admission is keyed by.
    specifier: String,
    name: String,
    version: String,
    /// The registry tarball's subresource integrity, from the host's lockfile.
    integrity: String,
    /// The file this specifier resolves to, relative to the installed package
    /// root -- `dist/index.js` or `dist/index.d.ts`. It is what selects between
    /// two acceptances that share a declaration file.
    resolved_target: String,
    /// The artifact snapshot root of the installed package's files
    /// (`installed_package_snapshot_root`), which a bundle's signed
    /// `snapshotRoot` must equal (ADR 0131): a lockfile keeps the published
    /// integrity for a patched package, so the integrity alone does not say the
    /// files are the certified archive's. Absent admits nothing.
    #[serde(default)]
    snapshot_root: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanRequest {
    project_id: String,
    #[serde(default)]
    dialect: Option<String>,
    generation: u64,
    sources: Vec<SourceFile>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanResponse {
    project_id: String,
    generation: u64,
    demands: Vec<typefacts::v3::EntityDemand>,
}

/// The host has already run TypeScript and hands the finished table in, so
/// there is no producer and no demand round trip: the one table answers the
/// single analysis this entry point performs.
struct InMemoryTypeFacts(Option<TypeScriptTable>);

impl TypeFactsProvider for InMemoryTypeFacts {
    fn semantic_grouped(
        &mut self,
        _groups: &[SemanticDemandGroup<'_>],
    ) -> Result<TypeScriptTable, BackendError> {
        self.0
            .take()
            .ok_or_else(|| BackendError::Process("TypeFacts response was already consumed".into()))
    }
}

/// Error text the planning provider ends the build with once the demand plan
/// is in hand. It is an internal signal, never a host-visible failure:
/// [`plan`] recognizes its own stop and answers with the captured plan.
const PLANNING_STOP: &str = "demand plan captured";

/// Records the demand plan the backend computes, then stops the build.
///
/// The backend has no plan-only entry point, and the demands exist for exactly
/// as long as the one round trip that would hand them to a producer. Ending
/// that round trip in an error is the cheap early stop: it skips the hydrate,
/// join, and `ProjectFacts` that planning would only discard, and it avoids
/// fabricating a Type Facts table whose project identity and source digests
/// nothing would ever certify.
#[derive(Default)]
struct PlanningTypeFacts {
    demands: Option<Vec<typefacts::v3::EntityDemand>>,
}

impl TypeFactsProvider for PlanningTypeFacts {
    fn semantic_grouped(
        &mut self,
        groups: &[SemanticDemandGroup<'_>],
    ) -> Result<TypeScriptTable, BackendError> {
        self.demands = Some(
            groups
                .iter()
                .flat_map(|group| group.demands.iter().cloned())
                .collect(),
        );
        Err(BackendError::Process(PLANNING_STOP.into()))
    }
}

/// Prototype seam for a self-contained browser host.
///
/// It runs the in-process syntax and Solid compiler passes and returns the
/// exact semantic demands a browser-side TypeScript-Go reactor must answer.
#[cfg(feature = "napi-host")]
#[napi]
pub fn plan_sync(request_json: String) -> napi::Result<String> {
    plan(&request_json).map_err(|error| napi::Error::from_reason(error.to_string()))
}

/// Known prototype approximation: a host that plans and then checks parses and
/// compiles the same sources twice, because the demand plan is a by-product of
/// the fact build rather than an output of its own backend entry point. The
/// stop below removes the Type Facts round trip and everything after it, not
/// the syntax and Solid compiler passes that precede it.
pub fn plan(request_json: &str) -> Result<String, Box<dyn std::error::Error>> {
    let request: PlanRequest = serde_json::from_str(request_json)?;
    let dialect = match request.dialect.as_deref() {
        None => solid_facts_backend::dialect::default_dialect(),
        Some(id) => solid_facts_backend::dialect::by_id(id)
            .ok_or_else(|| format!("unknown dialect {id:?}"))?,
    };
    let mut typescript = PlanningTypeFacts::default();
    let built = build_project_native_measured_with_demands(
        dialect,
        request.project_id.clone(),
        request.generation,
        request.sources,
        &mut typescript,
        SemanticDemandOptions::PREFERENCES,
    )
    .map(|(facts, _)| facts);
    let demands = match (typescript.demands, built) {
        // The planned stop: the demands were captured, so the error that ended
        // the build is this function's own signal rather than a failure.
        (Some(demands), _) => demands,
        (None, Err(error)) => return Err(error.into()),
        (None, Ok(_)) => return Err("the fact build requested no Type Facts".into()),
    };
    Ok(serde_json::to_string(&PlanResponse {
        project_id: request.project_id,
        generation: request.generation,
        demands,
    })?)
}

/// Analyze an in-memory project without spawning native processes.
///
/// The host supplies the TypeFacts closure produced by the browser-side
/// TypeScript engine. The result is the same JSON snapshot emitted by the CLI.
#[cfg(feature = "napi-host")]
#[napi]
pub fn check_sync(request_json: String) -> napi::Result<String> {
    check(&request_json).map_err(|error| napi::Error::from_reason(error.to_string()))
}

pub fn check(request_json: &str) -> Result<String, Box<dyn std::error::Error>> {
    let request: CheckRequest = serde_json::from_str(request_json)?;
    let dialect = match request.dialect.as_deref() {
        None => solid_facts_backend::dialect::default_dialect(),
        Some(id) => solid_facts_backend::dialect::by_id(id)
            .ok_or_else(|| format!("unknown dialect {id:?}"))?,
    };
    let mut typescript = InMemoryTypeFacts(Some(request.type_facts));
    let (facts, _) = build_project_native_measured_with_demands(
        dialect,
        request.project_id.clone(),
        request.generation,
        request.sources.clone(),
        &mut typescript,
        SemanticDemandOptions::PREFERENCES,
    )?;
    // P5 of the contract-removal plan: this request supplies no package files
    // and no import attestation, so source analysis cannot run here. Every
    // package import keeps an open requirement (SC9005) rather than reading as
    // plain code.
    let boundary = solid_facts_backend::unsupplied_package_boundary(&facts);
    let contracts = solid_facts_backend::requirement_index(&boundary);
    let (analysis, _) = analyze_project_inputs_measured_with_enablement(
        dialect,
        Path::new(&request.project_id),
        &request.sources,
        &facts,
        AnalysisInputs {
            boundary: &boundary,
            contracts: &contracts,
            hosts: None,
        },
        Default::default(),
    )?;
    Ok(serde_json::to_string(&analysis.snapshot)?)
}

#[cfg(test)]
mod tests {
    use super::CheckRequest;

    /// The adapter's answer through the shared input is the contracts-only
    /// answer: an empty boundary moves nothing.
    #[test]
    fn the_shared_input_with_an_empty_boundary_answers_as_contracts_alone() {
        let request = r#"{"projectId":"/p/tsconfig.json","generation":1,"sources":[],
                "typeFacts":{"schema":2,"generation":1,"projectId":"/p/tsconfig.json",
                "sources":[],"entities":[],"symbols":[],"files":[]}}"#;
        let shared = super::check(request).expect("checks");
        let decoded: CheckRequest = serde_json::from_str(request).unwrap();
        let dialect = solid_facts_backend::dialect::default_dialect();
        let mut typescript = super::InMemoryTypeFacts(Some(decoded.type_facts));
        let (facts, _) = solid_facts_backend::build_project_native_measured_with_demands(
            dialect,
            decoded.project_id.clone(),
            decoded.generation,
            decoded.sources.clone(),
            &mut typescript,
            solid_facts_backend::SemanticDemandOptions::PREFERENCES,
        )
        .unwrap();
        let (legacy, _) = solid_facts_backend::analyze_project_accepted_measured_with_enablement(
            dialect,
            std::path::Path::new(&decoded.project_id),
            &decoded.sources,
            &facts,
            &Default::default(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(shared, serde_json::to_string(&legacy.snapshot).unwrap());
    }

    /// P5: with no package files and no attestation, a package import is an
    /// open requirement, never plain code.
    #[test]
    fn a_package_import_stays_an_open_requirement() {
        let source = "import { createSignal } from \"solid-js\";\nimport { watch } from \"reactive-package\";\nexport function App() {\n  const [count] = createSignal(0);\n  watch(() => count());\n  return null;\n}\n";
        let request = serde_json::json!({
            "projectId": "/p/tsconfig.json",
            "generation": 1,
            "sources": [{ "path": "/p/App.tsx", "source": source }],
            "typeFacts": {
                "schema": 3, "generation": 1, "projectId": "/p/tsconfig.json",
                "sources": [{
                    "path": "/p/App.tsx",
                    "sha256": "sha256:a9c198f434cf2a82aec1fcc4bf3b8dfdd66b697b37d491cdd15f92dda85df77c"
                }],
                "entities": [], "symbols": [], "files": []
            }
        });
        let snapshot = super::check(&request.to_string()).expect("checks");
        assert!(snapshot.contains("SC9005"), "{snapshot}");
    }

    /// A request from a host written for the retired compiled-in tier still
    /// decodes: its fields are accepted and have no effect (ADR 0228).
    #[test]
    fn a_request_stating_an_installed_tree_still_decodes() {
        let stated: Result<CheckRequest, _> = serde_json::from_str(
            r#"{"projectId":"/p/tsconfig.json","generation":1,"sources":[],
                "typeFacts":{"schema":2,"generation":1,"projectId":"/p/tsconfig.json",
                "sources":[],"entities":[],"symbols":[],"files":[]},
                "exportConditions":["import"],"bundledContracts":true,
                "installedPackages":[{"specifier":"@scope/pkg","name":"@scope/pkg",
                "version":"1.0.0","integrity":"sha512-x","resolvedTarget":"dist/index.js",
                "snapshotRoot":"sha256:00"}]}"#,
        );
        assert!(stated.is_ok());
    }
}

#[cfg(feature = "reactor")]
static REACTOR_INPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

#[cfg(feature = "reactor")]
static REACTOR_OUTPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Allocate the request buffer used by the process-free WASI reactor.
#[cfg(feature = "reactor")]
#[unsafe(no_mangle)]
pub extern "C" fn allocate_input(size: u32) -> u32 {
    let mut input = REACTOR_INPUT.lock().expect("reactor input lock poisoned");
    input.resize(size as usize, 0);
    input.as_mut_ptr() as u32
}

#[cfg(feature = "reactor")]
fn run_reactor(operation: fn(&str) -> Result<String, Box<dyn std::error::Error>>) -> u32 {
    let result = {
        let input = REACTOR_INPUT.lock().expect("reactor input lock poisoned");
        std::str::from_utf8(&input)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
            .and_then(operation)
    };
    let (status, encoded) = match result {
        Ok(response) => (0, response.into_bytes()),
        Err(error) => (
            1,
            serde_json::to_vec(&serde_json::json!({ "error": error.to_string() }))
                .expect("serialize reactor error"),
        ),
    };
    *REACTOR_OUTPUT.lock().expect("reactor output lock poisoned") = encoded;
    status
}

/// Plan the exact Type Facts demands for the encoded source request.
#[cfg(feature = "reactor")]
#[unsafe(no_mangle)]
pub extern "C" fn run_plan() -> u32 {
    run_reactor(plan)
}

/// Analyze the encoded source and Type Facts request.
#[cfg(feature = "reactor")]
#[unsafe(no_mangle)]
pub extern "C" fn run_check() -> u32 {
    run_reactor(check)
}

/// Check a project from its files alone: the shared ordinary check, with the
/// canonical producer reached through the host's owned reactor and both
/// modules reading the same mounted project tree. No package contract is
/// consumed from any tier.
#[cfg(all(feature = "reactor", target_family = "wasm"))]
#[unsafe(no_mangle)]
pub extern "C" fn run_owned_check() -> u32 {
    run_reactor(owned::check)
}

/// The project the owned check analyzes, named as the native check names it:
/// the native command line canonicalizes `--project` before anything reads it,
/// and every later reader sees only that canonical path. WASI resolves links
/// and `..` physically against the reactor's filesystem -- a mount's closed
/// tree, or in directory mode the host's own namespace, read through the
/// owned store -- so this is the same walk over the same bytes. It does not
/// rewrite a name the host matched case-insensitively, as macOS
/// `realpath(3)` does (a held difference). A relative path is refused rather
/// than resolved: a reactor has no working directory for it to be relative
/// to.
#[cfg(any(test, all(feature = "reactor", target_family = "wasm")))]
fn canonical_project(project_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let project = Path::new(project_id);
    if !project.is_absolute() {
        return Err(format!("project {project_id} is not an absolute path").into());
    }
    Ok(project.canonicalize()?.to_string_lossy().into_owned())
}

#[cfg(test)]
mod canonical_project_tests {
    use super::canonical_project;

    fn scratch(name: &str) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("solid-checker-wasm-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("project/node_modules")).unwrap();
        std::fs::write(root.join("project/tsconfig.json"), "{}\n").unwrap();
        root.canonicalize().unwrap()
    }

    /// Every spelling native `--project` accepts names the one canonical file.
    #[test]
    fn a_linked_or_dotted_project_is_named_by_its_canonical_path() {
        let root = scratch("spellings");
        let canonical = root.join("project/tsconfig.json");
        #[cfg(unix)]
        std::os::unix::fs::symlink("project", root.join("alias")).unwrap();
        let mut spellings = vec![
            canonical.clone(),
            root.join("project/node_modules/../tsconfig.json"),
            root.join("project/./tsconfig.json"),
        ];
        if cfg!(unix) {
            spellings.push(root.join("alias/tsconfig.json"));
        }
        for spelling in spellings {
            assert_eq!(
                canonical_project(&spelling.to_string_lossy()).unwrap(),
                canonical.to_string_lossy(),
                "{}",
                spelling.display()
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_relative_project_is_refused_and_a_missing_one_fails_as_native_does() {
        assert_eq!(
            canonical_project("tsconfig.json").unwrap_err().to_string(),
            "project tsconfig.json is not an absolute path"
        );
        let root = scratch("missing");
        let missing = root.join("project/absent/tsconfig.json");
        let error = canonical_project(&missing.to_string_lossy()).unwrap_err();
        assert_eq!(
            error.to_string(),
            missing.canonicalize().unwrap_err().to_string()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(feature = "reactor")]
#[unsafe(no_mangle)]
pub extern "C" fn output_pointer() -> u32 {
    REACTOR_OUTPUT
        .lock()
        .expect("reactor output lock poisoned")
        .as_ptr() as u32
}

#[cfg(feature = "reactor")]
#[unsafe(no_mangle)]
pub extern "C" fn output_length() -> u32 {
    REACTOR_OUTPUT
        .lock()
        .expect("reactor output lock poisoned")
        .len() as u32
}

/// The owned check (S1 of the WASM host protocol).
///
/// The host owns a canonical Type Facts producer reactor and lends this module
/// six imports from `solid_typefacts`. Every call is synchronous and none
/// calls back into this module: the host copies bytes in and out of this
/// module's memory directly, at the pointers given, during the call.
///
/// - `open(envelope, length, handle_out) -> status` opens one program from the
///   CBOR open envelope and writes its handle (u32 little endian) at
///   `handle_out`.
/// - `handshake(handle) -> status` and `exchange(handle, frame, length) ->
///   status` leave one result per handle: the startup frame, or exactly one
///   response frame for exactly one request frame.
/// - `result_length(handle)` and `copy_result(handle, destination, capacity)
///   -> status` read that result into this module's memory and release it.
/// - `close(handle) -> status` releases the program.
///
/// A nonzero status means the result is a UTF-8 message (for a failed open,
/// held under handle 0). The host is trusted to forward the producer's bytes
/// unchanged and to keep the mounted tree immutable during a check; nothing
/// here authenticates either.
#[cfg(all(feature = "reactor", target_family = "wasm"))]
mod owned {
    use std::{error::Error, path::Path};

    use serde::{Deserialize, Serialize};
    use solid_facts_backend::{
        RuntimeEnvironment, Snapshot, TypeFactsSession,
        check::{self, ProducerSessions},
        dependency_sources::PreparedDependencyProgram,
    };

    #[link(wasm_import_module = "solid_typefacts")]
    unsafe extern "C" {
        fn open(envelope: *const u8, length: u32, handle: *mut u32) -> u32;
        fn handshake(handle: u32) -> u32;
        fn exchange(handle: u32, frame: *const u8, length: u32) -> u32;
        fn result_length(handle: u32) -> u32;
        fn copy_result(handle: u32, destination: *mut u8, capacity: u32) -> u32;
        fn close(handle: u32) -> u32;
    }

    fn take_result(handle: u32) -> Result<Vec<u8>, String> {
        // SAFETY: the host writes at most `capacity` bytes into the buffer it
        // is handed, during this call only.
        let length = unsafe { result_length(handle) };
        let mut buffer = vec![0_u8; length as usize];
        let status = unsafe { copy_result(handle, buffer.as_mut_ptr(), length) };
        if status != 0 {
            return Err(format!("host could not copy the result of handle {handle}"));
        }
        Ok(buffer)
    }

    fn answer(handle: u32, status: u32) -> Result<Vec<u8>, String> {
        let bytes = take_result(handle)?;
        if status == 0 {
            Ok(bytes)
        } else {
            Err(String::from_utf8_lossy(&bytes).into_owned())
        }
    }

    /// One program the host opened, carried frame by frame.
    struct HostBridge {
        handle: u32,
        closed: bool,
    }

    impl typefacts::FrameBridge for HostBridge {
        fn handshake(&mut self) -> Result<Vec<u8>, String> {
            // SAFETY: a plain call into the host; no memory is shared.
            let status = unsafe { handshake(self.handle) };
            answer(self.handle, status)
        }

        fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, String> {
            let length = u32::try_from(request.len()).map_err(|_| "request frame too large")?;
            // SAFETY: the host reads `length` bytes at `request` during this
            // call only.
            let status = unsafe { exchange(self.handle, request.as_ptr(), length) };
            answer(self.handle, status)
        }

        fn close(&mut self) {
            if !self.closed {
                self.closed = true;
                // SAFETY: a plain call into the host.
                unsafe { close(self.handle) };
            }
        }

        fn instance(&self) -> String {
            format!("owned-reactor-{}", self.handle)
        }
    }

    impl Drop for HostBridge {
        fn drop(&mut self) {
            typefacts::FrameBridge::close(self);
        }
    }

    struct OwnedSessions;

    impl OwnedSessions {
        fn open_program(
            project_id: &str,
            config: Option<&[u8]>,
        ) -> Result<TypeFactsSession, Box<dyn Error>> {
            let envelope = typefacts::owned_open_envelope(project_id, config)?;
            let length = u32::try_from(envelope.len()).map_err(|_| "open envelope too large")?;
            let mut handle = 0_u32;
            // SAFETY: the host reads `length` bytes at `envelope` and writes
            // four bytes at `handle`, during this call only.
            let status = unsafe { open(envelope.as_ptr(), length, &raw mut handle) };
            if status != 0 {
                return Err(answer(0, status)
                    .err()
                    .unwrap_or_else(|| "producer program did not open".into())
                    .into());
            }
            Ok(TypeFactsSession::open_bridged(
                Box::new(HostBridge {
                    handle,
                    closed: false,
                }),
                project_id,
            )?)
        }
    }

    impl ProducerSessions for OwnedSessions {
        fn open(&mut self, project_id: &str) -> Result<TypeFactsSession, Box<dyn Error>> {
            Self::open_program(project_id, None)
        }

        fn open_generated(
            &mut self,
            program: &PreparedDependencyProgram,
        ) -> Result<Result<(String, TypeFactsSession), String>, Box<dyn Error>> {
            // The generated program is an overlay at its logical path, never a
            // file written into the mounted project.
            let logical = program.logical_path()?.to_string_lossy().into_owned();
            let session = Self::open_program(&logical, Some(&program.document))?;
            Ok(Ok((logical, session)))
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct OwnedCheckRequest {
        project_id: String,
        #[serde(default)]
        dialect: Option<String>,
        #[serde(default = "first_generation")]
        generation: u64,
        #[serde(default)]
        runtime: RuntimeEnvironment,
        #[serde(default)]
        presets: Vec<String>,
        #[serde(default)]
        enable_rules: Vec<String>,
        #[serde(default)]
        dependency_sources: Vec<String>,
        #[serde(default)]
        no_dependency_source: bool,
        #[serde(default)]
        window_realm: bool,
    }

    const fn first_generation() -> u64 {
        1
    }

    /// The snapshot in its own field order, beside the notes the native check
    /// writes to standard error.
    #[derive(Serialize)]
    struct OwnedCheckResponse<'a> {
        snapshot: &'a Snapshot,
        notes: &'a [String],
    }

    pub(super) fn check(request_json: &str) -> Result<String, Box<dyn Error>> {
        let mut request: OwnedCheckRequest = serde_json::from_str(request_json)?;
        if request.no_dependency_source {
            return Err(
                "noDependencySource was removed: package contracts are no longer \
                 consumed, so a package is analyzed from source or its behaviour stays \
                 uncertifiable"
                    .into(),
            );
        }
        // From here on the request names the project only by its canonical
        // path, exactly as the native check's request does; a missing project
        // fails here as it does natively.
        request.project_id = super::canonical_project(&request.project_id)?;
        let project = Path::new(&request.project_id);
        let mut notes = Vec::<String>::new();
        let mut note = |line: &str| notes.push(line.to_owned());
        let (dialect, unsupported) = check::select_dialect(project, request.dialect.as_deref())?;
        if let Some(unsupported) = unsupported {
            if request.runtime.target.is_none() {
                note(&format!(
                    "solid-checker: note: browser host not inferred: {}: unsupported installed Solid runtime {}",
                    unsupported.manifest.display(),
                    unsupported.installed
                ));
            }
            let snapshot = solid_facts_backend::unsupported_runtime_snapshot(
                &unsupported.installed,
                &unsupported.manifest,
                unsupported.refusal,
            );
            return Ok(serde_json::to_string(&OwnedCheckResponse {
                snapshot: &snapshot,
                notes: &notes,
            })?);
        }
        // ADR 0297, exactly as the native ordinary check decides it.
        let default_applies = request.runtime.target.is_some()
            && request.runtime.build.is_some()
            && request.dependency_sources.is_empty();
        let (dependency_sources, implicit_dependency_sources) =
            check::effective_dependency_selection(
                project,
                &request.dependency_sources,
                default_applies,
                request.window_realm,
                &mut note,
            )?;
        let options = check::Options {
            project_id: request.project_id.clone(),
            dialect,
            generation: request.generation,
            sources: Vec::new(),
            runtime: request.runtime,
            presets: request.presets,
            enable_rules: request.enable_rules,
            feedback_facts: false,
            dependency_sources,
            implicit_dependency_sources,
            window_realm: request.window_realm,
            runtime_resolution: false,
        };
        let outcome = check::run(&options, &mut OwnedSessions, &mut note)?;
        Ok(serde_json::to_string(&OwnedCheckResponse {
            snapshot: &outcome.analysis.snapshot,
            notes: &notes,
        })?)
    }
}
