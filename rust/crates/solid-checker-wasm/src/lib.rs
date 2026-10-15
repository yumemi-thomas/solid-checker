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
    AcceptedContractSource, AnalysisInputs, BackendError, PackageBoundary, ResolvedImport,
    SemanticDemandOptions, SourceFile, TypeFactsProvider,
    analyze_project_inputs_measured_with_enablement, build_project_native_measured_with_demands,
    load_external_contract_index,
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
    /// Receipt-validated stable-v1 contracts paired with the host's full
    /// exact artifact-resolution records. Missing entries remain
    /// uncertifiable; this boundary has no name-only compatibility path.
    #[serde(default)]
    accepted_contracts: Vec<HostAcceptedContract>,
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
struct HostAcceptedContract {
    /// Exact UTF-8 document bytes. A parsed JSON value cannot preserve the
    /// wire digest bound by the receipt.
    document: String,
    /// Exact UTF-8 receipt bytes.
    receipt: String,
    import: ResolvedImport,
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
    let encoded = request
        .accepted_contracts
        .into_iter()
        .map(|contract| {
            (
                contract.document.into_bytes(),
                contract.receipt.into_bytes(),
                contract.import,
            )
        })
        .collect::<Vec<_>>();
    let contracts =
        load_external_contract_index(encoded.iter().map(|(document, receipt, import)| {
            AcceptedContractSource {
                document,
                receipt,
                import,
            }
        }))?;
    // The shared analysis input. This request states no unanalyzable package,
    // so the boundary is empty: an import nothing answers stays an ordinary
    // uncertifiable path, exactly as before the boundary existed. A host
    // protocol that states one is phase P5's (contract-removal plan).
    let boundary = PackageBoundary::default();
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

/// Testable internal seam for the future atomic policy-2 cut. It is not a
/// Node-API export and the active WASM request still accepts policy 1 only.
#[doc(hidden)]
pub fn authenticate_policy2_contract_for_wasm(
    canonical_main: &[u8],
    receipt: &[u8],
    expected: &solid_facts_backend::Policy2ReceiptBindings,
    provenance: solid_facts_backend::Policy2ReceiptProvenance<'_>,
) -> Result<
    solid_facts_backend::AuthenticatedPolicy2Receipt,
    solid_facts_backend::Policy2ReceiptError,
> {
    solid_facts_backend::authenticate_policy2_receipt(canonical_main, receipt, expected, provenance)
}

#[cfg(test)]
mod policy2_receipt_tests {
    use std::collections::BTreeMap;

    use solid_facts_backend::{
        ConfiguredReceiptIssuer, Policy2ReceiptBindings, Policy2ReceiptProvenance,
        Policy2TrustEntry, Policy2TrustStore, ReceiptIssuerKind, canonicalize_policy2_main,
        issue_policy2_receipt, policy2_main_semantic_digest, policy2_policy_digest,
    };

    use super::authenticate_policy2_contract_for_wasm;

    const MAIN: &[u8] = include_bytes!("../../../../pkg/contracts/bundled/solid-v2/solid-js.json");
    const OTHER: &[u8] =
        include_bytes!("../../../../pkg/contracts/bundled/solid-v2/solidjs-signals.json");

    fn root(value: u8) -> String {
        format!("sha256:{value:064x}")
    }

    fn witness_roots() -> BTreeMap<String, String> {
        [
            "package-identity",
            "manifest-entrypoint",
            "export-resolution",
            "artifact-declarations",
            "export-identity",
            "module-closure",
            "selected-signature",
            "argument-binding",
            "rest-spread-coverage",
            "callable-path",
            "operation-reachability",
            "operation-cardinality",
            "recursive-value-shape",
            "guard-partition",
            "compiler-reconciliation",
            "accepted-dependency-composition",
            "domain-exhaustiveness",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, family)| (family.into(), root((index + 20) as u8)))
        .collect()
    }

    fn bindings(main: &[u8]) -> Policy2ReceiptBindings {
        Policy2ReceiptBindings {
            importer: "/workspace/src/App.tsx".into(),
            specifier: "fixture-package".into(),
            resolved_import_root: root(0),
            artifact_acceptance_root: root(19),
            semantic_digest: policy2_main_semantic_digest(main).unwrap(),
            artifact_provenance_root: root(1),
            snapshot_root: root(2),
            package_root: root(3),
            manifest_root: root(4),
            artifacts_root: root(5),
            declarations_root: root(6),
            transform_root: root(7),
            exports_root: root(8),
            closure_root: root(9),
            demand_graph_root: root(10),
            verified_positive_root: root(11),
            witness_roots: witness_roots(),
            producer_sessions_root: root(13),
            dependency_receipts_root: root(14),
            dependency_trust_root: root(15),
            probe_gate_root: root(16),
            closed_claims_root: root(17),
            verifier_source_digest: root(18),
            verifier_build_digest: root(19),
            dependency_environment_root: String::new(),
            cited_acceptances: Vec::new(),
        }
    }

    fn trust(
        issuer: &ConfiguredReceiptIssuer,
        bindings: &Policy2ReceiptBindings,
    ) -> Policy2TrustStore {
        Policy2TrustStore::new(
            [Policy2TrustEntry {
                kind: ReceiptIssuerKind::Portable,
                key_id: issuer.key_id().into(),
                public_key: issuer.public_key(),
                scope: issuer.scope().into(),
                allowed_policy_digests: vec![policy2_policy_digest().into()],
                allowed_verifier_builds: vec![bindings.verifier_build_digest.clone()],
                revoked_at_epoch: None,
            }],
            1,
        )
        .unwrap()
    }

    #[test]
    fn wasm_internal_loader_authenticates_only_a_configured_portable_root() {
        let main = canonicalize_policy2_main(MAIN).unwrap();
        let bindings = bindings(&main);
        let issuer = ConfiguredReceiptIssuer::portable("wasm-release-chain", [31; 32]).unwrap();
        let receipt = issue_policy2_receipt(&main, &bindings, &issuer).unwrap();
        let trust = trust(&issuer, &bindings);
        authenticate_policy2_contract_for_wasm(
            &main,
            &receipt,
            &bindings,
            Policy2ReceiptProvenance::Portable {
                trust_store: &trust,
            },
        )
        .unwrap();
    }

    #[test]
    fn wasm_internal_loader_rejects_cross_main_receipt_replay() {
        let main = canonicalize_policy2_main(MAIN).unwrap();
        let other = canonicalize_policy2_main(OTHER).unwrap();
        let bindings = bindings(&main);
        let issuer = ConfiguredReceiptIssuer::portable("wasm-release-chain", [32; 32]).unwrap();
        let receipt = issue_policy2_receipt(&main, &bindings, &issuer).unwrap();
        let trust = trust(&issuer, &bindings);
        assert!(
            authenticate_policy2_contract_for_wasm(
                &other,
                &receipt,
                &bindings,
                Policy2ReceiptProvenance::Portable {
                    trust_store: &trust,
                },
            )
            .is_err()
        );
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
/// and `..` against the mounted tree, so this is the same operation over the
/// same bytes. A relative path is refused rather than resolved: a reactor has
/// no working directory for it to be relative to.
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
        check::{self, ContractConsumption, ProducerSessions},
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
            && request.dependency_sources.is_empty()
            && !request.no_dependency_source;
        let (dependency_sources, implicit_dependency_sources) =
            check::effective_dependency_selection(
                project,
                &request.dependency_sources,
                request.no_dependency_source,
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
            consumption: ContractConsumption::Disabled,
            bundled_contracts: false,
            accepted_contract_catalog: String::new(),
            receipt_trust_configuration: String::new(),
        };
        let outcome = check::run(&options, &mut OwnedSessions, &mut note)?;
        Ok(serde_json::to_string(&OwnedCheckResponse {
            snapshot: &outcome.analysis.snapshot,
            notes: &notes,
        })?)
    }
}
