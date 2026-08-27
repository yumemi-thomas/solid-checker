use sha2::{Digest, Sha256};
use solid_facts_backend::{
    ArtifactResolutionFailure, ArtifactResolver, BundledEvidenceStore, ContractFailure,
    EvidenceKey, EvidenceStore, EvidenceStoreFailure, HostResolutionAdapter, ImportRequest,
    LocalEvidenceStore, ResolutionAuthority, ResolutionTraceStep, ResolvedFile, ResolvedImport,
    StandaloneResolutionAdapter, load_accepted_contract,
};
use std::sync::Arc;
use std::{fs, time::SystemTime};

fn resolved() -> ResolvedImport {
    ResolvedImport {
        specifier: "solid-js".into(),
        importer: "/project/src/app.tsx".into(),
        requested_entrypoint: ".".into(),
        package_name: "solid-js".into(),
        package_version: "2.0.0-rc.3".into(),
        package_integrity: "sha512:registry-integrity".into(),
        package_manifest: "/project/node_modules/solid-js/package.json".into(),
        runtime: ResolvedFile {
            path: "/project/node_modules/solid-js/dist/solid.js".into(),
            digest: format!("sha256:{}", "1".repeat(64)),
        },
        declarations: ResolvedFile {
            path: "/project/node_modules/solid-js/types/index.d.ts".into(),
            digest: format!("sha256:{}", "2".repeat(64)),
        },
        dependency_closure_digest: format!("sha256:{}", "3".repeat(64)),
        transform: None,
        export_trace: vec![ResolutionTraceStep {
            condition: "import".into(),
            target: "./dist/solid.js".into(),
        }],
        authority: ResolutionAuthority::HostTypeFacts,
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn development_document() -> &'static [u8] {
    br#"{"format":"solid-reactivity-contract","schemaVersion":2,"semanticModelVersion":1,"package":{"name":"solid-js","version":"2.0.0-rc.3","integrity":"sha512:test","manifest":{"path":"package.json","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}},"summaries":{"plain":{"shape":"plain"}},"entrypoints":{".":{"artifact":{"path":"dist/solid.js","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","closureSha256":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"},"declarations":{"path":"types/index.d.ts","sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"},"exports":{"version":"plain"}}},"sidecars":{}}"#
}

#[test]
fn malformed_documents_fail_through_the_single_loading_interface() {
    assert!(matches!(
        load_accepted_contract(b"not json", b"{}", &resolved()),
        Err(ContractFailure::DocumentDecode { .. })
    ));
}

#[test]
fn normalized_development_schema_stays_fail_closed_until_acceptance_lands() {
    let document = development_document();
    let receipt = format!(
        "{{\"receiptVersion\":1,\"wireDigest\":\"{}\",\"semanticModelVersion\":1,\"semanticDigest\":\"sha256:{zeros}\",\"artifactsDigest\":\"sha256:{zeros}\",\"closureDigest\":\"sha256:{zeros}\",\"proofRoot\":\"sha256:{zeros}\",\"closedClaimsRoot\":\"sha256:{zeros}\",\"verifier\":{{\"build\":\"phase-2-test\",\"policy\":1}}}}",
        digest(document),
        zeros = "0".repeat(64),
    );

    assert!(matches!(
        load_accepted_contract(document, receipt.as_bytes(), &resolved()),
        Err(ContractFailure::AcceptanceUnavailable)
    ));
}

#[test]
fn replacement_contract_requires_the_format_discriminator() {
    let document = br#"{"schemaVersion":2,"semanticModelVersion":1,"package":{"name":"solid-js","version":"2.0.0-rc.3","integrity":"sha512:test"},"entrypoints":{".":{"artifact":{},"exports":{}}}}"#;
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
fn a_stale_receipt_is_rejected_before_normalization() {
    let document = development_document();
    let zeros = "0".repeat(64);
    let receipt = format!(
        "{{\"receiptVersion\":1,\"wireDigest\":\"sha256:{zeros}\",\"semanticModelVersion\":1,\"semanticDigest\":\"sha256:{zeros}\",\"artifactsDigest\":\"sha256:{zeros}\",\"closureDigest\":\"sha256:{zeros}\",\"proofRoot\":\"sha256:{zeros}\",\"closedClaimsRoot\":\"sha256:{zeros}\",\"verifier\":{{\"build\":\"phase-2-test\",\"policy\":1}}}}"
    );

    assert!(matches!(
        load_accepted_contract(document, receipt.as_bytes(), &resolved()),
        Err(ContractFailure::ReceiptMismatch {
            field: "wireDigest"
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
        ResolutionAuthority::HostTypeFacts
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
