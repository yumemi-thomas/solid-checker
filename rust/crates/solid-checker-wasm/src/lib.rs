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
    AcceptedContractSource, BackendError, ResolvedImport, SemanticDemandOptions, SourceFile,
    TypeFactsProvider, analyze_project_accepted_measured_with_enablement,
    build_project_native_measured_with_demands, load_accepted_contract_index,
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
        load_accepted_contract_index(encoded.iter().map(|(document, receipt, import)| {
            AcceptedContractSource {
                document,
                receipt,
                import,
            }
        }))?;
    let (analysis, _) = analyze_project_accepted_measured_with_enablement(
        dialect,
        Path::new(&request.project_id),
        &request.sources,
        &facts,
        &contracts,
        Default::default(),
    )?;
    Ok(serde_json::to_string(&analysis.snapshot)?)
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

    const MAIN: &[u8] =
        include_bytes!("../../../../pkg/contracts/bundled/solid-v1/debounce-root-default.json");
    const OTHER: &[u8] =
        include_bytes!("../../../../pkg/contracts/bundled/solid-v1/solid-root-node.json");

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
