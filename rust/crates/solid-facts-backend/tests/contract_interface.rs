use sha2::{Digest, Sha256};
use solid_facts::compiler::{AnalysisRequest, CompilerOptions, ExecutionMap};
use solid_facts_backend::{
    ArtifactResolutionFailure, ArtifactResolver, BundledEvidenceStore, ClosureManifest,
    ContractFailure, EvidenceKey, EvidenceStore, EvidenceStoreFailure, HostResolutionAdapter,
    ImportRequest, LocalEvidenceStore, ReceiptStore, ResolutionAuthority, ResolutionTrace,
    ResolvedExportBinding, ResolvedExportTarget, ResolvedFile, ResolvedImport,
    StandaloneResolutionAdapter, load_accepted_contract,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
    time::SystemTime,
};

fn resolved() -> ResolvedImport {
    let root = "/project/node_modules/solid-js";
    let runtime = ResolvedFile {
        path: format!("{root}/dist/solid.js"),
        real_path: None,
        digest: format!("sha256:{}", "b".repeat(64)),
    };
    let declarations = ResolvedFile {
        path: format!("{root}/types/index.d.ts"),
        real_path: None,
        digest: format!("sha256:{}", "d".repeat(64)),
    };
    ResolvedImport {
        specifier: "solid-js".into(),
        importer: "/project/src/app.tsx".into(),
        requested_entrypoint: ".".into(),
        package_name: "solid-js".into(),
        package_version: "2.0.0-rc.3".into(),
        package_integrity: "sha512:test".into(),
        package_root: root.into(),
        package_real_root: None,
        package_manifest: ResolvedFile {
            path: format!("{root}/package.json"),
            real_path: None,
            digest: format!("sha256:{}", "a".repeat(64)),
        },
        runtime: runtime.clone(),
        declarations: declarations.clone(),
        runtime_trace: ResolutionTrace::default(),
        declaration_trace: ResolutionTrace::default(),
        closure: ClosureManifest::new(vec![], vec![], vec![]).unwrap(),
        transform: None,
        exports: BTreeMap::from([(
            "version".into(),
            ResolvedExportBinding {
                runtime: ResolvedExportTarget {
                    module: runtime,
                    export_name: "version".into(),
                },
                declarations: ResolvedExportTarget {
                    module: declarations,
                    export_name: "version".into(),
                },
            },
        )]),
        authority: ResolutionAuthority::Host,
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[test]
fn compiler_certification_child_binds_the_live_pid_request_and_materialized_output() {
    fn launch(nonce: &str) -> (u32, serde_json::Value) {
        let analysis = AnalysisRequest::new(
            "input.tsx",
            "const view = <div>{count()}</div>;",
            CompilerOptions::default(),
        );
        let request = serde_json::json!({
            "protocol": 1,
            "nonce": nonce,
            "snapshotRoot": format!("sha256:{}", "1".repeat(64)),
            "demandGraphRoot": format!("sha256:{}", "2".repeat(64)),
            "demandId": format!("sha256:{}", "3".repeat(64)),
            "analysis": analysis,
        });
        let mut child = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
            .arg("--internal-compiler-certification-session")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let process_id = child.id();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(&request).unwrap())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        (process_id, serde_json::from_slice(&output.stdout).unwrap())
    }

    let (first_pid, first) = launch("session-one");
    let (second_pid, second) = launch("session-two");
    for (pid, nonce, response) in [
        (first_pid, "session-one", &first),
        (second_pid, "session-two", &second),
    ] {
        assert_eq!(response["processId"], pid);
        assert_eq!(response["nonce"], nonce);
        assert_eq!(
            response["compilerIdentity"],
            "solid-v2:trace3:7f4e1135943c1fb01231d1bda707b4a1856a5607"
        );
        assert_eq!(
            response["compilerSourceManifestSha256"],
            "sha256:613049ba60fa514c662bd9350adb4b0ed9c3031e4f80f2bd1ecb23d56846fde0"
        );
        let map: ExecutionMap = serde_json::from_value(response["executionMap"].clone()).unwrap();
        assert!(map.semantic_model.source_operations_complete);
        assert!(!map.semantic_model.generated_operations_complete);
        assert!(!map.semantic_model.operations.is_empty());
        assert_eq!(
            map.semantic_model.producer.unwrap().output_sha256,
            format!(
                "{:x}",
                Sha256::digest(response["output"].as_str().unwrap().as_bytes())
            )
        );
    }
    assert_eq!(first["requestSha256"], second["requestSha256"]);
}

fn development_document() -> Vec<u8> {
    let closure = resolved().closure.digest;
    format!(
        "{{\"format\":\"solid-reactivity-contract\",\"schemaVersion\":1,\"semanticModelVersion\":1,\"package\":{{\"name\":\"solid-js\",\"version\":\"2.0.0-rc.3\",\"integrity\":\"sha512:test\",\"manifest\":{{\"path\":\"package.json\",\"sha256\":\"{}\"}}}},\"summaries\":{{\"plain\":{{\"shape\":\"plain\"}}}},\"entrypoints\":{{\".\":{{\"artifact\":{{\"path\":\"dist/solid.js\",\"sha256\":\"{}\",\"closureSha256\":\"{}\"}},\"declarations\":{{\"path\":\"types/index.d.ts\",\"sha256\":\"{}\"}},\"exports\":{{\"version\":\"plain\"}}}}}},\"sidecars\":{{}}}}",
        "a".repeat(64),
        "b".repeat(64),
        closure.trim_start_matches("sha256:"),
        "d".repeat(64),
    )
    .into_bytes()
}

#[test]
fn malformed_documents_fail_through_the_single_loading_interface() {
    assert!(matches!(
        load_accepted_contract(b"not json", b"{}", &resolved()),
        Err(ContractFailure::DocumentDecode { .. })
    ));
}

#[test]
fn normalized_stable_schema_reports_policy1_receipts_as_obsolete() {
    let document = development_document();
    let receipt = format!(
        "{{\"receiptVersion\":1,\"wireDigest\":\"{}\",\"semanticModelVersion\":1,\"semanticDigest\":\"sha256:{zeros}\",\"artifactsDigest\":\"sha256:{zeros}\",\"closureDigest\":\"sha256:{zeros}\",\"proofRoot\":\"sha256:{zeros}\",\"closedClaimsRoot\":\"sha256:{zeros}\",\"verifier\":{{\"build\":\"phase-2-test\",\"policy\":1}}}}",
        digest(&document),
        zeros = "0".repeat(64),
    );

    let result = load_accepted_contract(&document, receipt.as_bytes(), &resolved());
    assert!(matches!(
        result,
        Err(ContractFailure::UnsupportedReceiptVersion {
            expected: 2,
            actual: 1
        })
    ));
}

#[test]
fn replacement_contract_requires_the_format_discriminator() {
    let document = br#"{"schemaVersion":1,"semanticModelVersion":1,"package":{"name":"solid-js","version":"2.0.0-rc.3","integrity":"sha512:test"},"entrypoints":{".":{"artifact":{},"exports":{}}}}"#;
    let receipt = format!(
        "{{\"receiptVersion\":1,\"wireDigest\":\"{}\",\"semanticModelVersion\":1,\"semanticDigest\":\"sha256:{zeros}\",\"artifactsDigest\":\"sha256:{zeros}\",\"closureDigest\":\"sha256:{zeros}\",\"proofRoot\":\"sha256:{zeros}\",\"closedClaimsRoot\":\"sha256:{zeros}\",\"verifier\":{{\"build\":\"phase-6-test\",\"policy\":1}}}}",
        digest(document),
        zeros = "0".repeat(64),
    );

    assert!(matches!(
        load_accepted_contract(document, receipt.as_bytes(), &resolved()),
        Err(ContractFailure::DocumentDecode { .. })
    ));
}

#[test]
fn policy1_stale_receipt_is_obsolete_before_any_binding_is_trusted() {
    let document = development_document();
    let zeros = "0".repeat(64);
    let receipt = format!(
        "{{\"receiptVersion\":1,\"wireDigest\":\"sha256:{zeros}\",\"semanticModelVersion\":1,\"semanticDigest\":\"sha256:{zeros}\",\"artifactsDigest\":\"sha256:{zeros}\",\"closureDigest\":\"sha256:{zeros}\",\"proofRoot\":\"sha256:{zeros}\",\"closedClaimsRoot\":\"sha256:{zeros}\",\"verifier\":{{\"build\":\"phase-2-test\",\"policy\":1}}}}"
    );

    assert!(matches!(
        load_accepted_contract(&document, receipt.as_bytes(), &resolved()),
        Err(ContractFailure::UnsupportedReceiptVersion {
            expected: 2,
            actual: 1
        })
    ));
}

#[test]
fn resolver_adapters_preserve_authority_and_refuse_duplicate_answers() {
    let request = ImportRequest {
        specifier: "solid-js".into(),
        importer: "/project/src/app.tsx".into(),
        export_conditions: vec!["import".into(), "development".into()],
    };
    let first = resolved();
    let host = HostResolutionAdapter::from_rows([(request.clone(), first.clone())]);
    assert_eq!(
        host.resolve(&request).unwrap().authority,
        ResolutionAuthority::Host
    );

    let standalone = StandaloneResolutionAdapter::from_rows([
        (request.clone(), first.clone()),
        (request.clone(), first),
    ]);
    assert_eq!(
        standalone.resolve(&request),
        Err(ArtifactResolutionFailure::Ambiguous)
    );
}

#[test]
fn evidence_stores_rehash_receipts_and_missing_local_entries_are_not_errors() {
    let bytes: Arc<[u8]> = Arc::from(&b"accepted receipt"[..]);
    let key = EvidenceKey::parse(digest(&bytes)).unwrap();
    let bundled = BundledEvidenceStore::new([(key.clone(), Arc::clone(&bytes))]);
    assert_eq!(
        bundled.receipt(&key).unwrap().as_deref(),
        Some(bytes.as_ref())
    );

    let wrong_key = EvidenceKey::parse(format!("sha256:{}", "0".repeat(64))).unwrap();
    let tampered = BundledEvidenceStore::new([(wrong_key.clone(), bytes)]);
    assert_eq!(
        tampered.receipt(&wrong_key),
        Err(EvidenceStoreFailure::ContentMismatch)
    );

    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "solid-checker-phase2-{}-{unique}",
        std::process::id()
    ));
    let local = LocalEvidenceStore::new(&root);
    assert!(local.receipt(&key).unwrap().is_none());

    let receipt_dir = root.join("receipts");
    fs::create_dir_all(&receipt_dir).unwrap();
    fs::write(
        receipt_dir.join(digest(b"accepted receipt").trim_start_matches("sha256:")),
        b"accepted receipt",
    )
    .unwrap();
    assert_eq!(
        local.receipt(&key).unwrap().as_deref(),
        Some(&b"accepted receipt"[..])
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_local_receipts_are_canonical_content_addressed_and_idempotent() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "solid-checker-phase11-receipts-{}-{unique}",
        std::process::id()
    ));
    let local = LocalEvidenceStore::new(&root);
    let bytes = b"canonical acceptance receipt\n";
    let first = local.store_receipt(bytes).unwrap();
    let second = local.store_receipt(bytes).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        local.receipt(&first).unwrap().as_deref(),
        Some(bytes.as_slice())
    );
    assert_eq!(first.as_str(), digest(bytes));

    let uppercase = EvidenceKey::parse(format!(
        "sha256:{}",
        first
            .as_str()
            .trim_start_matches("sha256:")
            .to_ascii_uppercase()
    ))
    .unwrap();
    assert_eq!(uppercase, first);
    fs::remove_dir_all(root).unwrap();
}
