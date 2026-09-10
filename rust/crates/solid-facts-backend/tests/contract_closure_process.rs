//! A closed package contract must light up the ordinary reactivity rules.
//!
//! This is the regression test the 2026-08-30 policy-2 cut left the repository
//! without. Every fixture catalog under `fixtures/` carries an obsolete
//! policy-1 receipt, so the checker's ability to diagnose *third-party*
//! reactive misuse — the product's central use case — has no live coverage:
//! see `docs/package-contract-v2/2026-09-10-findings-delta-measurement.md`.
//!
//! What this test scopes, and what it deliberately does not:
//!
//! - It exercises the **consumer** half of the pipeline: an authenticated
//!   policy-2 receipt over a contract whose demanded domains are closed, and
//!   the rules that then fire on a misuse of the package's reactive accessor.
//! - It says nothing about whether *certification* can currently produce such
//!   a contract. It cannot: `reads` is closed for 0 of 8950 corpus exports, so
//!   every real certified import raises SC9005 today. Closure is supplied here
//!   by the fixture's hand-authored document, which is exactly the input a
//!   future closure lever is supposed to produce.
//!
//! The issuer is test-scoped in the only way that matters: the trust
//! configuration is written by this test into a temporary tree and handed to
//! the checker out of band. A project cannot nominate its own issuer
//! (`contract_interface.rs`: "Trust bytes are deliberately not referenced by
//! the project catalog"), the key exists only for the duration of the test,
//! and nothing signed here is committed.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::support::{decode_findings, temporary_directory};
use solid_facts_backend::{
    ConfiguredReceiptIssuer, Policy2ReceiptBindings, Policy2ReceiptProvenance,
    RECEIPT_WITNESS_FAMILIES, ResolvedImport, authenticate_policy2_receipt,
    canonicalize_policy2_main, encode_policy2_trust_configuration, issue_policy2_receipt,
    policy2_main_closed_claims_root, policy2_main_semantic_digest, policy2_resolved_import_root,
    policy2_trust_configuration_for_issuer, publish_policy2_catalog,
};

/// The fixture is reused rather than duplicated: its `App.tsx` already pairs a
/// tracked read with an untracked one over an accessor whose reactivity is
/// established *only* by the package contract, and its document already closes
/// `callbacks`, `reads`, `creates` and `returns` — the conjunction
/// `push_unknown_contract_claims` requires.
const FIXTURE: &str = "fixtures/reactive-ir/package-return-consumer";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// Shape-valid stand-ins for roots whose authority comes from the verifier
/// sessions that would supply them in a real certification. `Policy2ReceiptBindings`
/// validates shape only, by design, and this test is about what the *consumer*
/// does with an authenticated receipt.
fn root(index: u16) -> String {
    format!("sha256:{index:064x}")
}

/// The stored catalog spells every path relative to the project, which is what
/// makes the fixture relocatable; `ResolvedImport::validate` requires absolute
/// ones. Rewrite exactly the project-relative fields against the temporary
/// copy, leaving package-relative closure entries alone.
fn absolutize(import: &mut serde_json::Value, project: &Path) {
    let at = |value: &serde_json::Value| -> String {
        project
            .join(value.as_str().expect("path is a string"))
            .to_string_lossy()
            .into_owned()
    };
    for key in ["importer", "packageRoot"] {
        import[key] = at(&import[key]).into();
    }
    for key in ["packageManifest", "runtime", "declarations"] {
        import[key]["path"] = at(&import[key]["path"]).into();
    }
    let Some(exports) = import["exports"].as_object_mut() else {
        return;
    };
    for binding in exports.values_mut() {
        for axis in ["runtime", "declarations"] {
            binding[axis]["module"]["path"] = project
                .join(binding[axis]["module"]["path"].as_str().unwrap())
                .to_string_lossy()
                .into_owned()
                .into();
        }
    }
}

/// Runs the fixture under a freshly minted policy-2 receipt, optionally
/// reopening one claim domain first, and returns the findings.
fn findings_under_a_minted_receipt(label: &str, reopen: Option<&str>) -> Vec<serde_json::Value> {
    mint_and_analyze(FIXTURE, label, reopen).expect("the reference fixture mints")
}

/// Every fixture tree that ships an accepted catalog and is analyzable — the
/// population whose imports could demand a contract domain at all.
fn catalog_bearing_fixtures() -> Vec<String> {
    fn walk(directory: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            if path
                .join(".solid-checker/accepted-contracts.json")
                .is_file()
                && path.join("tsconfig.json").is_file()
            {
                found.push(path.clone());
            }
            walk(&path, found);
        }
    }
    let root = repository_root();
    let mut found = Vec::new();
    walk(&root.join("fixtures"), &mut found);
    let mut relative = found
        .iter()
        .filter_map(|path| path.strip_prefix(&root).ok())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>();
    relative.sort();
    relative
}

/// Mints a policy-2 receipt over a fixture's own accepted catalog and returns
/// the findings the checker then produces.
///
/// The authorization is replaced and nothing else: the catalog's `import`
/// block is reused verbatim, so this swaps an obsolete policy-1 receipt for a
/// test-scoped policy-2 one without touching the resolution the fixture pins.
fn mint_and_analyze(
    fixture: &str,
    label: &str,
    reopen: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("caller guards on the producer");

    let project = temporary_directory(label).join("consumer");
    copy_tree(&repository_root().join(fixture), &project);
    // The catalog reader canonicalizes every path it rebases, and the receipt
    // binds the importer it will compute. On macOS `env::temp_dir()` is a
    // symlink (`/var` -> `/private/var`), so an uncanonicalized root here binds
    // a path the consumer never derives.
    let project = fs::canonicalize(&project).unwrap();

    // The fixture's catalog already carries a valid `import` block; only its
    // policy-1 authorization is obsolete. Reuse the resolver answer verbatim so
    // this test replaces the *authorization*, not the resolution.
    let catalog_path = project.join(".solid-checker/accepted-contracts.json");
    let catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(&catalog_path).unwrap()).unwrap();
    let contracts = catalog["contracts"].as_array().expect("a contracts array");
    if contracts.len() != 1 {
        // `publish_policy2_catalog` writes a catalog holding exactly one
        // contract, so a fixture pinning several cannot be minted through the
        // public publication path. Reported rather than worked around: a
        // hand-assembled multi-entry catalog would be this test asserting its
        // own idea of the on-disk shape.
        return Err(format!(
            "catalog publishes {} contracts; publication writes one",
            contracts.len()
        ));
    }
    let entry = &contracts[0];
    if entry["status"] != "obsolete-policy1" {
        return Err(format!("catalog status is {}", entry["status"]));
    }
    let mut import = entry["import"].clone();
    absolutize(&mut import, &project);
    let resolved: ResolvedImport = serde_json::from_value(import).unwrap();
    let document = project.join(entry["document"].as_str().unwrap());
    // `"<domain>-items"` strips the domain's positive operations instead of
    // reopening it: the claim stays closed, but over nothing. Reopening and
    // stripping are different questions — one removes the proof that an
    // enumeration is complete, the other removes the enumeration — and
    // conflating them is how a measurement of the first got reported as the
    // second.
    if let Some(domain) = reopen.and_then(|value| value.strip_suffix("-items")) {
        let mut contract: serde_json::Value =
            serde_json::from_slice(&fs::read(&document).unwrap()).unwrap();
        for summary in contract["summaries"].as_object_mut().unwrap().values_mut() {
            let call = summary["call"].as_object_mut().expect("a call object");
            let removed = call
                .get(domain)
                .and_then(|items| items.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            call.insert(domain.to_owned(), serde_json::json!([]));
            if let Some(operations) = call.get_mut("operations").and_then(|v| v.as_array_mut()) {
                operations.retain(|operation| {
                    operation["id"]
                        .as_str()
                        .is_none_or(|id| !removed.iter().any(|name| name == id))
                });
            }
        }
        fs::write(&document, serde_json::to_vec(&contract).unwrap()).unwrap();
    } else if let Some(domain) = reopen {
        // Reopening one domain in the fixture's own document is how this file
        // asks whether *partial* closure is worth anything: every sibling
        // domain stays closed and usable.
        let mut contract: serde_json::Value =
            serde_json::from_slice(&fs::read(&document).unwrap()).unwrap();
        for summary in contract["summaries"].as_object_mut().unwrap().values_mut() {
            let closed = summary["call"]["closed"].as_array_mut().unwrap();
            closed.retain(|value| value != domain);
            // An open domain may not carry an empty item collection — that
            // shape is a *closed* claim proving absence, and leaving it
            // behind makes the document undecodable. Reopening means dropping
            // the enumeration too; a non-empty one is positive knowledge the
            // open domain still legitimately states.
            if summary["call"][domain]
                .as_array()
                .is_some_and(Vec::is_empty)
            {
                summary["call"]
                    .as_object_mut()
                    .expect("a call object")
                    .remove(domain);
            }
        }
        fs::write(&document, serde_json::to_vec(&contract).unwrap()).unwrap();
    }
    let canonical_main = canonicalize_policy2_main(&fs::read(&document).unwrap()).unwrap();

    let bindings = Policy2ReceiptBindings {
        importer: resolved.importer.clone(),
        specifier: resolved.specifier.clone(),
        resolved_import_root: policy2_resolved_import_root(&resolved).unwrap(),
        semantic_digest: policy2_main_semantic_digest(&canonical_main).unwrap(),
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
        witness_roots: RECEIPT_WITNESS_FAMILIES
            .iter()
            .enumerate()
            .map(|(index, family)| {
                (
                    (*family).to_owned(),
                    root(u16::try_from(100 + index).unwrap()),
                )
            })
            .collect(),
        producer_sessions_root: root(12),
        dependency_receipts_root: root(13),
        dependency_trust_root: root(14),
        probe_gate_root: root(15),
        // Rebound by the consumer from the document itself: a test issuer
        // cannot assert a closure the contract does not carry.
        closed_claims_root: policy2_main_closed_claims_root(&canonical_main).unwrap(),
        verifier_source_digest: root(17),
        verifier_build_digest: root(18),
    };

    let issuer = ConfiguredReceiptIssuer::persistent_local("solid-checker-fixture", [7u8; 32])
        .expect("test-scoped issuer");
    let receipt = issue_policy2_receipt(&canonical_main, &bindings, &issuer).unwrap();
    let trust = policy2_trust_configuration_for_issuer(&issuer, &bindings.verifier_build_digest, 0)
        .unwrap();
    let authenticated = authenticate_policy2_receipt(
        &canonical_main,
        &receipt,
        &bindings,
        Policy2ReceiptProvenance::PersistentLocal {
            trust_store: trust.trust_store(),
            scope: issuer.scope(),
        },
    )
    .expect("the test issuer's own receipt authenticates under its own trust");

    // Publication replaces the catalog atomically; the obsolete pointer must be
    // gone rather than merged with.
    fs::remove_file(&catalog_path).unwrap();
    publish_policy2_catalog(
        &project.join(".solid-checker"),
        &canonical_main,
        &receipt,
        &authenticated,
        &resolved,
    )
    .expect("publish the freshly authorized catalog");

    // Out of band, exactly as an ordinary analysis requires: the project never
    // names its own issuer.
    let trust_path = project.join("fixture-trust.json");
    fs::write(
        &trust_path,
        encode_policy2_trust_configuration(&trust).unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .args([
            "--project",
            &project.join("tsconfig.json").to_string_lossy(),
            "--typefacts",
            &typefacts,
            "--receipt-trust-configuration",
            &trust_path.to_string_lossy(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    if !output.status.success() {
        return Err(format!(
            "analysis refused: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(decode_findings(&output.stdout))
}

/// Separates two things a contract's `reads` claim carries, because
/// conflating them produced a wrong answer once already: the **items** it
/// enumerates, and the **proof that the enumeration is complete**.
///
/// The question behind it: `reads` cannot close in bulk — no synthesized veto
/// can observe a read of a source the export owns, so every closure needs a
/// hand recipe. If what rules actually need is the items, and only SC9005
/// needs the completeness, then scoping that one obligation delivers every
/// rule its facts without a single recipe.
///
/// Reopening the domain drops the completeness and keeps the items; stripping
/// the operations drops the items and keeps the closure. Measuring both is
/// what tells them apart.
///
/// Measured by difference. Each project is analyzed twice against its own
/// minted catalog, once as the contract stands and once with `reads` reopened
/// in the document. `package-contract-incomplete` is excluded from the
/// comparison because it demands the domain by definition — it exists to
/// report that a claim is open, so reopening one always moves it and it says
/// nothing about whether a *rule's proof* depended on the claim.
#[test]
fn reads_completeness_is_demanded_only_by_sc9005_while_its_items_feed_rules() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let incomplete = |rule: &str| rule.contains("package-contract-incomplete");
    let proven = |findings: &[serde_json::Value]| {
        let mut rules = findings
            .iter()
            .filter_map(|finding| finding["rule"].as_str())
            .filter(|rule| !incomplete(rule))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        rules.sort();
        rules
    };
    let mut consulted = Vec::new();
    let mut indifferent = Vec::new();
    let mut inert = Vec::new();
    let mut consumes_items = Vec::new();
    for (index, fixture) in catalog_bearing_fixtures().iter().enumerate() {
        let closed = mint_and_analyze(fixture, &format!("reads-closed-{index}"), None);
        let reopened = mint_and_analyze(fixture, &format!("reads-open-{index}"), Some("reads"));
        let stripped = mint_and_analyze(
            fixture,
            &format!("reads-items-{index}"),
            Some("reads-items"),
        );
        let (Ok(closed), Ok(reopened)) = (closed, reopened) else {
            continue;
        };
        if let Ok(stripped) = stripped
            && proven(&closed) != proven(&stripped)
        {
            consumes_items.push(fixture.clone());
        }
        // The control for the whole measurement. Reopening `reads` must move
        // *something*, or the mutation did not take and the project would
        // read as indifferent for the wrong reason. SC9005 is what must move:
        // it reports open claims by name, so an open `reads` has to reach it.
        // A project where it does not is excluded rather than counted —
        // `package-unknown-export` imports an export the contract does not
        // describe, so it is already uncertifiable for a reason reopening a
        // claim cannot change.
        let incomplete_count = |findings: &[serde_json::Value]| {
            findings
                .iter()
                .filter_map(|finding| finding["rule"].as_str())
                .filter(|rule| incomplete(rule))
                .count()
        };
        if incomplete_count(&reopened) <= incomplete_count(&closed) {
            inert.push(fixture.clone());
            continue;
        }

        let (before, after) = (proven(&closed), proven(&reopened));
        if before == after {
            indifferent.push(fixture.clone());
        } else {
            consulted.push((fixture.clone(), before.len(), after.len()));
        }
    }
    println!(
        "reads demand: {} of {} projects consult the projection ({} excluded, reopen inert)",
        consulted.len(),
        consulted.len() + indifferent.len(),
        inert.len()
    );
    for fixture in &inert {
        println!("  excluded    {fixture}");
    }
    println!(
        "reads items: {} project(s) change when the read operations are removed",
        consumes_items.len()
    );
    for fixture in &consumes_items {
        println!("  consumes    {fixture}");
    }
    for (fixture, before, after) in &consulted {
        println!("  consults    {fixture}  {before} -> {after} rule findings");
    }
    for fixture in &indifferent {
        println!("  indifferent {fixture}");
    }
}

/// The policy-2 fixture corpus: every catalog-bearing fixture that can be
/// minted, minted.
///
/// Before this, all twenty accepted catalogs in the tree carried
/// `obsolete-policy1` receipts, which are rejected before any claim is read —
/// so the corpus contained no call site at which a contract domain was ever
/// demanded, and the findings-delta measurement could only use one retained
/// real consumer. See
/// `docs/package-contract-v2/phase21/2026-09-10-reads-demand-population.md`.
///
/// What this asserts is narrow and deliberate: each fixture either mints and
/// analyzes, or reports why it cannot. It does not pin per-fixture findings —
/// those belong to the snapshots, which record the *obsolete* state and are
/// not what this replaces.
#[test]
fn the_catalog_bearing_fixtures_mint_a_policy_2_corpus() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let fixtures = catalog_bearing_fixtures();
    assert!(
        fixtures.len() >= 16,
        "the catalog-bearing population should not shrink silently: {fixtures:?}"
    );
    let mut minted = Vec::new();
    let mut unminted = Vec::new();
    for (index, fixture) in fixtures.iter().enumerate() {
        let label = format!("policy2-corpus-{index}");
        match mint_and_analyze(fixture, &label, None) {
            Ok(findings) => {
                let obsolete = findings.iter().any(|finding| {
                    finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("obsolete-policy1"))
                });
                assert!(
                    !obsolete,
                    "{fixture}: minted catalog still reports the obsolete-policy rejection"
                );
                let mut rules = findings
                    .iter()
                    .map(|finding| finding["rule"].as_str().unwrap_or("?").to_owned())
                    .collect::<Vec<_>>();
                rules.sort();
                minted.push((fixture.clone(), rules));
            }
            Err(reason) => unminted.push((fixture.clone(), reason)),
        }
    }
    let incomplete = |rule: &String| rule.contains("package-contract-incomplete");
    let uncertifiable = minted
        .iter()
        .flat_map(|(_, rules)| rules)
        .filter(|rule| incomplete(rule))
        .count();
    let proven = minted
        .iter()
        .flat_map(|(_, rules)| rules)
        .filter(|rule| !incomplete(rule))
        .count();
    println!(
        "policy-2 corpus: {} minted, {} unminted; {proven} rule findings, {uncertifiable} SC9005",
        minted.len(),
        unminted.len()
    );

    // Pinned, because the composition is the measurement. Before these
    // catalogs were authorized the same fourteen projects produced 16 rule
    // findings and 28 SC9005, all of the latter the obsolete-policy rejection
    // path; a regression that quietly returned them to that state would
    // otherwise pass. Update these deliberately, with the reason, exactly as
    // a snapshot update is made.
    assert_eq!(
        (minted.len(), unminted.len(), proven, uncertifiable),
        (14, 2, 32, 10),
        "policy-2 corpus composition moved; review before repinning"
    );

    // A catalog-bearing fixture exists to exercise a contract. If authorizing
    // its catalog changes nothing about what the checker reports, the fixture
    // is testing the *absence* of contract effect — which is legitimate for
    // some, and for the rest is the signature of the staleness this corpus
    // was built to end: the receipts were cut on 2026-08-30 and for eleven
    // days every one of these fixtures kept passing while its contract did
    // nothing.
    //
    // A snapshot cannot express this. It records what the checker said, so a
    // fixture whose contract went inert simply had its inertness recorded as
    // expected. This asserts what the fixture is *for*.
    let contract_makes_no_difference = |fixture: &str| match fixture {
        // The import names an export the contract does not describe, so it is
        // outside the contract with or without a valid receipt.
        "fixtures/reactive-ir/package-unknown-export" => {
            Some("imports an export the contract does not describe")
        }
        // A tsconfig `paths` entry shadows the installed package, so the
        // contract legitimately does not apply to the resolved module.
        "fixtures/reactive-ir/package-contract-paths-shadow" => {
            Some("a paths entry shadows the installed package")
        }
        _ => None,
    };
    for (fixture, rules) in &minted {
        let name = fixture.rsplit('/').next().unwrap_or_default();
        let group = fixture
            .strip_prefix("fixtures/")
            .and_then(|rest| rest.split('/').next())
            .unwrap_or_default();
        let snapshot = repository_root()
            .join("fixtures/findings-snapshots")
            .join(format!("{group}__{name}.json"));
        let Ok(bytes) = fs::read(&snapshot) else {
            continue;
        };
        let recorded: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let mut unauthorized = recorded["findings"]
            .as_array()
            .map(|findings| {
                findings
                    .iter()
                    .map(|finding| finding["rule"].as_str().unwrap_or("?").to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        unauthorized.sort();
        match contract_makes_no_difference(fixture) {
            Some(reason) => assert_eq!(
                &unauthorized, rules,
                "{fixture}: excused as \"{reason}\", but authorizing its catalog did change \
                 the findings — the excuse is stale"
            ),
            None => assert_ne!(
                &unauthorized, rules,
                "{fixture}: authorizing the catalog changed nothing, so this fixture is not \
                 exercising its contract"
            ),
        }
    }
    for (fixture, rules) in &minted {
        println!("  minted   {fixture}  [{}]", rules.join(", "));
    }
    for (fixture, reason) in &unminted {
        println!("  unminted {fixture}  {reason}");
    }
}

fn rules(findings: &[serde_json::Value]) -> Vec<&str> {
    findings
        .iter()
        .map(|finding| finding["rule"].as_str().unwrap_or_default())
        .collect()
}

#[test]
fn a_closed_contract_lets_the_rules_diagnose_third_party_reactive_misuse() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = findings_under_a_minted_receipt("contract-closure", None);

    // The point of the test: an authorized, fully closed contract is consumed
    // as evidence rather than reported as missing.
    assert!(
        !rules(&findings).contains(&"package-contract-incomplete"),
        "a closed, authenticated contract still reported missing claims: {findings:#?}"
    );
    let untracked: Vec<&serde_json::Value> = findings
        .iter()
        .filter(|finding| finding["rule"] == "strict-read-untracked")
        .collect();
    assert_eq!(
        untracked.len(),
        1,
        "exactly the untracked read of the package's accessor is a violation: {findings:#?}"
    );
    assert_eq!(untracked[0]["kind"], "violation");
}

/// Partial closure has to be worth something, or no incremental closure work
/// can ever pay off: today `reads` is closed for 0 of 8950 corpus exports, so
/// every real contract is in exactly this state for at least one domain.
///
/// `returns` is the domain reopened here because it is the one this fixture
/// states positively: reopening it leaves the accessor item in place while
/// making the collection non-exhaustive, which is exactly the partial-positive
/// state a real contract reaches. A domain the document states as an *empty*
/// collection cannot be reopened at all — the document validator refuses
/// "open domain call operation claim has an empty collection" — so partial
/// knowledge always means "these items, and maybe more".
///
/// The verified positive item must still prove what it proves, and the
/// obligation must name only the domain that is open.
#[test]
fn a_partially_closed_contract_still_proves_what_its_closed_domains_prove() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = findings_under_a_minted_receipt("contract-partial", Some("returns"));

    let untracked: Vec<&serde_json::Value> = findings
        .iter()
        .filter(|finding| finding["rule"] == "strict-read-untracked")
        .collect();
    assert_eq!(
        untracked.len(),
        1,
        "a partially known `returns` still proves the accessor identity: {findings:#?}"
    );
    assert_eq!(untracked[0]["kind"], "violation");

    let obligations: Vec<&str> = findings
        .iter()
        .filter(|finding| finding["rule"] == "package-contract-incomplete")
        .map(|finding| finding["analysisContext"].as_str().unwrap_or_default())
        .collect();
    assert!(
        !obligations.is_empty(),
        "an open domain must still be reported: {findings:#?}"
    );
    for context in &obligations {
        assert_eq!(
            *context, "unknown-contract-claims:returns",
            "the obligation must name only the domain that is open: {findings:#?}"
        );
    }
}

/// An open `returns` is reported only against a binding some consumer can
/// actually read it from.
///
/// The fixture's two exports carry a byte-identical contract and the same
/// summary id; the only difference is where the consumer puts them, so any
/// difference in what is reported is attributable to the use position alone.
///
/// - `createCount`'s result is bound at module scope, so
///   `docs/package-contract-v2/phase21/2026-09-10-sc9005-demand-scoping-design.md`
///   § 8's four consumers can still reach its `returns`. Reported.
/// - `createLabel` is called as a whole statement and is never an argument, so
///   `CallFact::result_discarded` is `true` at its only reference and **no
///   consumer can reach its `returns` at all**. Not reported.
///
/// This assertion was written in its inverted form *before* the slice, while
/// both were reported, so the change landed as a visible flip here rather than
/// as a claim in a commit message.
#[test]
fn an_open_returns_is_reported_only_where_a_consumer_can_read_it() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let findings = findings_under_a_minted_receipt("contract-demand", Some("returns"));

    let reported = |export: &str| {
        findings.iter().any(|finding| {
            finding["rule"] == "package-contract-incomplete"
                && finding["analysisContext"] == "unknown-contract-claims:returns"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains(export))
        })
    };

    assert!(
        reported("createCount"),
        "the bound result keeps its obligation: {findings:#?}"
    );
    assert!(
        !reported("createLabel"),
        "a discarded result no consumer reads must shed the obligation; \
         reporting it is noise, not a fail-closed answer: {findings:#?}"
    );
}

/// Where a `reads` item actually produces its finding.
///
/// This is the fact step 1 of the layer move turns on. The route sketched in
/// [the demand-scoping design § 12] proposed deferring SC9005's `reads`
/// conjunct to a layer that knows whether the call site is inside a
/// *tracking scope*, reasoning that an unenumerated read is only observable
/// where it would be tracked and "outside one it changes nothing any rule
/// proves".
///
/// `package-consumer` calls the same contracted accessor from two positions:
/// `Good` reads it inside JSX, and `Bad` binds it in the component body,
/// which is `ExecutionRole::UntrackedRendering`. The finding the contract's
/// read item produces is at `Bad` — the checker's own evidence line for it
/// reads "the call is outside every compiler-tracked JSX region and deferred
/// callback". So the demand is not confined to tracking scopes, and § 13
/// records the decision that follows.
///
/// Measured by difference against the same catalog with the read operations
/// stripped, so the finding is attributed to the item rather than to the
/// fixture merely being analyzable.
///
/// [the demand-scoping design § 12]: ../../../../docs/package-contract-v2/phase21/2026-09-10-sc9005-demand-scoping-design.md
#[test]
fn a_contract_read_is_consumed_outside_a_tracking_scope() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    const CONSUMER: &str = "fixtures/reactive-ir/package-consumer";
    let closed = mint_and_analyze(CONSUMER, "reads-site-closed", None)
        .expect("the read-stating fixture mints");
    let stripped = mint_and_analyze(CONSUMER, "reads-site-stripped", Some("reads-items"))
        .expect("the read-stating fixture mints with its items stripped");

    let source = fs::read_to_string(repository_root().join(CONSUMER).join("App.tsx")).unwrap();
    // The two call sites, located in the fixture text rather than pinned as
    // byte offsets, so editing the fixture cannot silently move the claim.
    let body = |name: &str| {
        let start = source.find(name).expect("the fixture declares it");
        let end = source[start..]
            .find("\n}\n")
            .map_or(source.len(), |offset| start + offset);
        start..end
    };
    let untracked_reads_in = |findings: &[serde_json::Value], range: &std::ops::Range<usize>| {
        findings
            .iter()
            .filter(|finding| finding["rule"] == "strict-read-untracked")
            .filter(|finding| {
                finding["primaryLocation"]["startByte"]
                    .as_u64()
                    .and_then(|start| usize::try_from(start).ok())
                    .is_some_and(|start| range.contains(&start))
            })
            .count()
    };

    let (good, bad) = (body("function Good"), body("function Bad"));
    assert_eq!(
        untracked_reads_in(&closed, &bad),
        1,
        "the contracted read bound in a component body is reported, and that \
         position is not a tracking scope: {closed:#?}"
    );
    assert_eq!(
        untracked_reads_in(&closed, &good),
        0,
        "the same accessor read inside JSX is tracked and reports nothing, \
         so the finding above is attributable to the position: {closed:#?}"
    );
    assert_eq!(
        untracked_reads_in(&stripped, &bad),
        0,
        "with the read operations stripped the contract states no read, so \
         the finding is the item's and not the fixture's: {stripped:#?}"
    );
}
