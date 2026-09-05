//! ADRs 0026, 0028 and 0030: execution-bound consumers with no ordinary acceptance token.

use std::collections::BTreeSet;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::{
    CertificationPlan, ConfiguredReceiptIssuer, Policy2ReceiptBindings, ProbeHarnessConfiguration,
    TypeFactsProducerPin, finalization, probe_harness, type_facts,
};

pub const INERT_EXECUTION_PROFILE: &str = "node-strip-inert-esm-v1";
pub const IMPORT_FREE_EXECUTION_PROFILE: &str = "node-strip-import-free-esm-v1";
pub const RELATIVE_GRAPH_EXECUTION_PROFILE: &str = "node-strip-relative-ts-graph-esm-v1";
const INERT_PROOF_IDENTITY: &str = "policy-2-with-inert-erasure-v1";
const IMPORT_FREE_PROOF_IDENTITY: &str = "policy-2-with-import-free-erasure-v1";
const RELATIVE_GRAPH_PROOF_IDENTITY: &str = "policy-2-with-relative-ts-graph-erasure-v1";
const SIGNATURE_DOMAIN: &[u8] = b"solid-checker:controlled-execution-receipt:v5\0";

pub(super) fn signature_message(payload: &[u8]) -> Vec<u8> {
    [SIGNATURE_DOMAIN, payload].concat()
}

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// Non-serializable parser evidence, always reconstructed from the authenticated
/// selected snapshot. Never construct this from a receipt or request.
#[derive(Clone)]
pub(super) struct InertModule {
    pub source_digest: String,
    pub output_digest: String,
    pub output: String,
    pub export_name: String,
    pub source_path: String,
    pub profile: &'static str,
    pub modules: Vec<DerivedModule>,
    pub edges: Vec<RelativeImportEdge>,
    preservation: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct DerivedModule {
    pub source_path: String,
    pub source_digest: String,
    pub output_digest: String,
    pub output: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(super) struct RelativeImportEdge {
    pub importer_path: String,
    pub specifier: String,
    pub target_path: String,
}

impl InertModule {
    pub(super) fn from_plan(
        plan: &CertificationPlan,
        profile: &str,
    ) -> Result<Self, ControlledExecutionError> {
        let path = plan.verified_resolution.runtime_path();
        if !path.ends_with(".ts") || path.ends_with(".d.ts") {
            return Err(ControlledExecutionError::Unsupported(
                "profile requires a .ts runtime member",
            ));
        }
        let source = plan
            .snapshot
            .read(path)
            .ok_or(ControlledExecutionError::Unsupported(
                "source absent from authenticated snapshot",
            ))?;
        let source_text = std::str::from_utf8(source)
            .map_err(|_| ControlledExecutionError::Unsupported("source is not UTF-8"))?;
        if !plan.verified_closure.manifest().dependencies.is_empty() {
            return Err(ControlledExecutionError::Unsupported(
                "runtime dependency closure is not empty",
            ));
        }
        let schedule = plan.probe_gate_schedule()?;
        if schedule.gates().len() != 1
            || !matches!(
                schedule.gates()[0].subject().path,
                super::SemanticClaimPath::Domain(super::ClaimPath::Call(
                    super::ClaimDomain::Creates
                ))
            )
        {
            return Err(ControlledExecutionError::Unsupported(
                "profile requires exactly one recipe-selected mandatory creates gate",
            ));
        }
        let target = schedule.gates()[0].subject().export.as_str();
        let (output, export_name, selected_profile, preservation) = match profile {
            INERT_EXECUTION_PROFILE => {
                let erased = solid_facts::ast::inert_erasure(source_text).ok_or(
                    ControlledExecutionError::Unsupported("module is outside the complete inert syntax whitelist (imports, reflection and executable expressions refuse)"))?;
                if erased.export_name != target {
                    return Err(ControlledExecutionError::Unsupported(
                        "inert module export differs from the selected gate",
                    ));
                }
                (
                    erased.output,
                    erased.export_name,
                    INERT_EXECUTION_PROFILE,
                    "inert-void-annotation-v1",
                )
            }
            IMPORT_FREE_EXECUTION_PROFILE => {
                let erased = solid_facts::ast::import_free_erasure(source_text, target).ok_or(
                    ControlledExecutionError::Unsupported(
                        "module is outside the complete import-free strip-only syntax whitelist",
                    ),
                )?;
                (
                    erased.output,
                    target.to_owned(),
                    IMPORT_FREE_EXECUTION_PROFILE,
                    "parser-runtime-token-preservation-v1",
                )
            }
            RELATIVE_GRAPH_EXECUTION_PROFILE => {
                return Self::from_relative_graph(plan, path, target);
            }
            _ => {
                return Err(ControlledExecutionError::Unsupported(
                    "unknown controlled execution profile",
                ));
            }
        };
        Ok(Self {
            source_digest: digest(source),
            output_digest: digest(output.as_bytes()),
            output,
            export_name,
            source_path: path.into(),
            profile: selected_profile,
            preservation,
            modules: Vec::new(),
            edges: Vec::new(),
        })
    }

    fn from_relative_graph(
        plan: &CertificationPlan,
        root_path: &str,
        target: &str,
    ) -> Result<Self, ControlledExecutionError> {
        use super::module_closure::{LocalResolution, ModuleAxis, resolve_local};
        use crate::artifact_resolution::ClosureFileRole;

        let runtime_paths = plan
            .verified_closure
            .manifest()
            .entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.role,
                    ClosureFileRole::Runtime | ClosureFileRole::LiteralDynamicChunk
                )
            })
            .map(|entry| {
                entry
                    .path
                    .strip_prefix("./")
                    .unwrap_or(&entry.path)
                    .to_owned()
            })
            .collect::<BTreeSet<_>>();
        if !runtime_paths.contains(root_path) {
            return Err(ControlledExecutionError::Unsupported(
                "selected runtime member is absent from the verified runtime closure",
            ));
        }

        let mut pending = vec![root_path.to_owned()];
        let mut visited = BTreeSet::new();
        let mut modules = Vec::new();
        let mut edges = BTreeSet::new();
        while let Some(path) = pending.pop() {
            if !visited.insert(path.clone()) {
                continue;
            }
            if !path.ends_with(".ts") || path.ends_with(".d.ts") {
                return Err(ControlledExecutionError::Unsupported(
                    "relative graph profile requires only .ts runtime modules",
                ));
            }
            let source = plan
                .snapshot
                .read(&path)
                .ok_or(ControlledExecutionError::Unsupported(
                    "relative graph source absent from authenticated snapshot",
                ))?;
            let source_text = std::str::from_utf8(source).map_err(|_| {
                ControlledExecutionError::Unsupported("relative graph source is not UTF-8")
            })?;
            let erased = solid_facts::ast::relative_import_erasure(
                source_text,
                (path == root_path).then_some(target),
            )
            .ok_or(ControlledExecutionError::Unsupported(
                "module is outside the complete relative-import strip-only syntax whitelist",
            ))?;
            for specifier in erased.imports {
                let LocalResolution::Module(imported) = resolve_local(
                    &plan.snapshot,
                    &path,
                    &specifier,
                    ModuleAxis::Runtime,
                )
                .map_err(|_| {
                    ControlledExecutionError::Unsupported(
                        "relative import cannot be normalized inside the authenticated snapshot",
                    )
                })?
                else {
                    return Err(ControlledExecutionError::Unsupported(
                        "relative import does not resolve to one authenticated runtime module",
                    ));
                };
                if !runtime_paths.contains(&imported) {
                    return Err(ControlledExecutionError::Unsupported(
                        "relative import target is absent from the verified runtime closure",
                    ));
                }
                edges.insert(RelativeImportEdge {
                    importer_path: path.clone(),
                    specifier,
                    target_path: imported.clone(),
                });
                pending.push(imported);
            }
            modules.push(DerivedModule {
                source_path: path,
                source_digest: digest(source),
                output_digest: digest(erased.output.as_bytes()),
                output: erased.output,
            });
        }
        if visited != runtime_paths {
            return Err(ControlledExecutionError::Unsupported(
                "relative graph does not exactly cover the verified runtime closure",
            ));
        }
        modules.sort_by(|left, right| left.source_path.cmp(&right.source_path));
        let edges = edges.into_iter().collect::<Vec<_>>();
        let root = modules
            .iter()
            .find(|module| module.source_path == root_path)
            .expect("the graph walk always materializes its root");
        Ok(Self {
            source_digest: root.source_digest.clone(),
            output_digest: root.output_digest.clone(),
            output: root.output.clone(),
            export_name: target.to_owned(),
            source_path: root_path.to_owned(),
            profile: RELATIVE_GRAPH_EXECUTION_PROFILE,
            modules,
            edges,
            preservation: "parser-runtime-token-preservation-v1",
        })
    }

    pub(super) fn binding(&self) -> Vec<String> {
        let mut binding = vec![
            format!("execution-profile:{}", self.profile),
            format!("source-path:{}", self.source_path),
            format!("source:{}", self.source_digest),
            format!("derived:{}", self.output_digest),
            format!("export:{}", self.export_name),
            format!(
                "module-format:esm;url:exact-private-source;runtime-imports:{};consumer:{}",
                if self.profile == RELATIVE_GRAPH_EXECUTION_PROFILE {
                    "authenticated-exact-relative-edge-map"
                } else {
                    "none"
                },
                if self.profile == INERT_EXECUTION_PROFILE {
                    "zero-argument-call"
                } else {
                    "recipe-replay"
                }
            ),
            format!(
                "transform:node-stripTypeScriptTypes;mode:strip;options:none;preservation:{}",
                self.preservation
            ),
        ];
        if self.profile == RELATIVE_GRAPH_EXECUTION_PROFILE {
            binding.push(format!(
                "derived-module-graph:{}",
                digest(&serde_json::to_vec(&self.modules).expect("derived modules serialize"))
            ));
            binding.push(format!(
                "relative-edge-map:{}",
                digest(&serde_json::to_vec(&self.edges).expect("relative edges serialize"))
            ));
            binding.push("resolver:native-authenticated-snapshot-runtime-v1".into());
        }
        binding
    }

    fn proof_identity(&self) -> &'static str {
        match self.profile {
            INERT_EXECUTION_PROFILE => INERT_PROOF_IDENTITY,
            IMPORT_FREE_EXECUTION_PROFILE => IMPORT_FREE_PROOF_IDENTITY,
            RELATIVE_GRAPH_EXECUTION_PROFILE => RELATIVE_GRAPH_PROOF_IDENTITY,
            _ => unreachable!("constructed profiles are closed"),
        }
    }
}

#[cfg(test)]
pub(super) fn assert_receipt_refusals(
    result: &ControlledExecution,
    issuer: &ConfiguredReceiptIssuer,
) {
    let expected = &result.receipt.payload;
    let receipt = serde_json::to_value(&result.receipt).unwrap();
    // Keep encoding canonical and the signature valid. Otherwise every
    // mutation could fail solely on JSON field order or a stale signature,
    // leaving the live consumer-binding comparison untested.
    let canonical_signed = |value: serde_json::Value| {
        let mut changed: Receipt = serde_json::from_value(value).unwrap();
        changed.signature = STANDARD.encode(
            issuer.sign_controlled_execution(&serde_json::to_vec(&changed.payload).unwrap()),
        );
        serde_json::to_vec(&changed).unwrap()
    };
    authenticate(&canonical_signed(receipt.clone()), expected, issuer).unwrap();
    for field in [
        "profile",
        "proofIdentity",
        "mainDigest",
        "issuerKeyId",
        "issuerScope",
    ] {
        let mut changed = receipt.clone();
        changed["payload"][field] = "mismatch".into();
        assert!(
            authenticate(&canonical_signed(changed), expected, issuer).is_err(),
            "{field}"
        );
    }
    let mut crossed = receipt.clone();
    crossed["payload"]["profile"] = if expected.profile == INERT_EXECUTION_PROFILE {
        IMPORT_FREE_EXECUTION_PROFILE
    } else {
        INERT_EXECUTION_PROFILE
    }
    .into();
    let crossed_bytes = canonical_signed(crossed);
    let crossed_receipt: Receipt = serde_json::from_slice(&crossed_bytes).unwrap();
    assert!(
        authenticate(&crossed_bytes, &crossed_receipt.payload, issuer).is_err(),
        "a valid profile name cannot be paired with the other profile's proof identity"
    );
    for field in [
        "transformRoot",
        "snapshotRoot",
        "probeGateRoot",
        "producerSessionsRoot",
    ] {
        let mut changed = receipt.clone();
        changed["payload"]["bindings"][field] = digest(b"mismatch").into();
        assert!(
            authenticate(&canonical_signed(changed), expected, issuer).is_err(),
            "{field}"
        );
    }
    for prefix in [
        "source:",
        "derived:",
        "node-executable:",
        "execution-profile:",
    ] {
        let mut changed = receipt.clone();
        let fields = changed["payload"]["executionBinding"]
            .as_array_mut()
            .unwrap();
        let field = fields
            .iter_mut()
            .find(|field| field.as_str().unwrap().starts_with(prefix))
            .unwrap();
        *field = format!("{prefix}{}", digest(b"mismatch")).into();
        assert!(
            authenticate(&canonical_signed(changed), expected, issuer).is_err(),
            "{prefix}"
        );
    }
    if expected.profile == RELATIVE_GRAPH_EXECUTION_PROFILE {
        for prefix in ["derived-module-graph:", "relative-edge-map:", "resolver:"] {
            let mut changed = receipt.clone();
            let fields = changed["payload"]["executionBinding"]
                .as_array_mut()
                .unwrap();
            let field = fields
                .iter_mut()
                .find(|field| field.as_str().unwrap().starts_with(prefix))
                .unwrap();
            *field = format!("{prefix}mismatch").into();
            assert!(
                authenticate(&canonical_signed(changed), expected, issuer).is_err(),
                "{prefix}"
            );
        }
    }
    let wrong_issuer = ConfiguredReceiptIssuer::persistent_local(issuer.scope(), [24; 32]).unwrap();
    assert!(
        authenticate(
            &serde_json::to_vec(&result.receipt).unwrap(),
            expected,
            &wrong_issuer
        )
        .is_err()
    );
    let trust = super::policy2_trust_configuration_for_issuer(
        issuer,
        &expected.bindings.verifier_build_digest,
        1,
    )
    .unwrap();
    let main = super::canonicalize_policy2_main(
        &serde_json::to_vec(&result.canonical_claim_document).unwrap(),
    )
    .unwrap();
    let error = super::authenticate_policy2_receipt(
        &main,
        &serde_json::to_vec(&result.receipt).unwrap(),
        &expected.bindings,
        super::Policy2ReceiptProvenance::PersistentLocal {
            trust_store: trust.trust_store(),
            scope: issuer.scope(),
        },
    )
    .expect_err("ordinary consumers must reject scoped controlled-execution receipts");
    assert!(
        matches!(error, super::Policy2ReceiptError::ObsoletePolicy),
        "{error}"
    );
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Payload {
    profile: String,
    proof_identity: String,
    base_policy_digest: String,
    main_digest: String,
    bindings: Policy2ReceiptBindings,
    execution_binding: Vec<String>,
    issuer_key_id: String,
    issuer_scope: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    format: String,
    receipt_version: u16,
    payload: Payload,
    signature: String,
}

/// A completed controlled invocation. It deliberately exposes no
/// AuthenticatedPolicy2Receipt, AcceptedContract, catalog, or replay capability.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledExecution {
    format: &'static str,
    schema_version: u16,
    profile: &'static str,
    consumer_outcome: &'static str,
    accepted_closures: usize,
    gate_ids: Vec<String>,
    canonical_claim_document: serde_json::Value,
    receipt: Receipt,
}

impl CertificationPlan {
    /// Native proof, mandatory derived-byte veto, then authenticated controlled
    /// consumption in a fresh private workspace. Receipt replay is unsupported.
    pub fn certify_and_execute(
        &self,
        profile: &str,
        proposal: &[u8],
        pin: &TypeFactsProducerPin,
        issuer: &ConfiguredReceiptIssuer,
        probes: &ProbeHarnessConfiguration,
    ) -> Result<ControlledExecution, ControlledExecutionError> {
        // Do not recipe-gate away the very closure the controlled consumer asks
        // to use. An absent recipe, incomplete gate or census is a refusal.
        let gated = self
            .recipe_gated(Some(probes.recipe_corpus()))
            .map_err(super::Policy2FinalizationError::from)?;
        let plan = gated.plan();
        let module = InertModule::from_plan(plan, profile)?;
        let evidence = type_facts::acquire_and_verify_export_values(plan, pin)
            .map_err(super::Policy2FinalizationError::from)?;
        let schedule = plan.probe_gate_schedule()?;
        let (evaluation, identity) =
            probe_harness::run_inert_gates(plan, &schedule, probes, pin, &module, false)?;
        let inspected =
            schedule.inspect_outcomes(schedule.outcomes_from_evaluation(&evaluation)?)?;
        let gates = schedule.authenticate_with_harness(inspected, &identity)?;
        // Regression at the issuance boundary: even complete, live-verified
        // evidence from this profile cannot produce an extractable v2 receipt.
        #[cfg(test)]
        assert!(matches!(
            finalization::finalize_value_only(plan, proposal, &evidence, &gates, pin, issuer, 0),
            Err(super::Policy2FinalizationError::ControlledExecutionRequired)
        ));
        let (main, mut bindings) =
            finalization::prepare_value_only(plan, proposal, Some(&evidence), None, &gates, pin)?;
        let execution_binding = identity
            .root_fields()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        bindings.transform_root = digest(&serde_json::to_vec(&module.binding())?);
        let payload = Payload {
            profile: module.profile.into(),
            proof_identity: module.proof_identity().into(),
            base_policy_digest: super::policy2_policy_digest().into(),
            main_digest: digest(&main),
            bindings,
            execution_binding,
            issuer_key_id: issuer.key_id().into(),
            issuer_scope: issuer.scope().into(),
        };
        let payload_bytes = serde_json::to_vec(&payload)?;
        let receipt = Receipt {
            format: "solid-checker-controlled-execution-receipt".into(),
            receipt_version: 5,
            signature: STANDARD.encode(issuer.sign_controlled_execution(&payload_bytes)),
            payload: payload.clone(),
        };
        let encoded = serde_json::to_vec(&receipt)?;
        authenticate(&encoded, &payload, issuer)?;
        // The capability above never leaves this stack. The consumer independently
        // rechecks all pins, copies, transformer bytes and resolution, and executes
        // the export itself (not a caller's consumer-compatibility assertion).
        let consumer_module = InertModule::from_plan(plan, profile)?;
        let (_, consumer_identity) =
            probe_harness::run_inert_gates(plan, &schedule, probes, pin, &consumer_module, true)?;
        if consumer_identity != identity {
            return Err(ControlledExecutionError::Mismatch(
                "consumer execution identity differs from the authenticated receipt",
            ));
        }
        Ok(ControlledExecution {
            format: "solid-checker-controlled-type-erasure-execution",
            schema_version: 1,
            profile: module.profile,
            consumer_outcome: if module.profile == INERT_EXECUTION_PROFILE {
                "completed-returned-undefined"
            } else {
                "completed-replayed-recipe"
            },
            accepted_closures: 1,
            gate_ids: gates.gate_ids().to_vec(),
            canonical_claim_document: serde_json::from_slice(&main)?,
            receipt,
        })
    }
}

fn authenticate(
    bytes: &[u8],
    expected: &Payload,
    issuer: &ConfiguredReceiptIssuer,
) -> Result<(), ControlledExecutionError> {
    if bytes.len() > 64 * 1024 {
        return Err(ControlledExecutionError::Mismatch("receipt byte budget"));
    }
    let receipt: Receipt = serde_json::from_slice(bytes)?;
    if receipt.receipt_version != 5
        || receipt.format != "solid-checker-controlled-execution-receipt"
        || receipt.payload != *expected
        || !matches!(
            (
                receipt.payload.profile.as_str(),
                receipt.payload.proof_identity.as_str()
            ),
            (INERT_EXECUTION_PROFILE, INERT_PROOF_IDENTITY)
                | (IMPORT_FREE_EXECUTION_PROFILE, IMPORT_FREE_PROOF_IDENTITY)
                | (
                    RELATIVE_GRAPH_EXECUTION_PROFILE,
                    RELATIVE_GRAPH_PROOF_IDENTITY
                )
        )
        || serde_json::to_vec(&receipt)? != bytes
    {
        return Err(ControlledExecutionError::Mismatch(
            "receipt profile, input, output, proof or canonical encoding mismatch",
        ));
    }
    let signature_bytes = STANDARD
        .decode(&receipt.signature)
        .map_err(|_| ControlledExecutionError::Mismatch("receipt signature encoding"))?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| ControlledExecutionError::Mismatch("receipt signature length"))?;
    let key = VerifyingKey::from_bytes(&issuer.public_key())
        .map_err(|_| ControlledExecutionError::Mismatch("configured issuer key"))?;
    key.verify_strict(
        &signature_message(&serde_json::to_vec(&receipt.payload)?),
        &signature,
    )
    .map_err(|_| ControlledExecutionError::Mismatch("receipt signature does not authenticate"))
}

#[derive(Debug, Error)]
pub enum ControlledExecutionError {
    #[error("controlled TypeScript erasure refused: {0}")]
    Unsupported(&'static str),
    #[error("controlled execution binding refused: {0}")]
    Mismatch(&'static str),
    #[error(transparent)]
    Proof(#[from] super::Policy2FinalizationError),
    #[error(transparent)]
    Gate(#[from] super::ProbeGateError),
    #[error(transparent)]
    Harness(#[from] super::ProbeHarnessError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
