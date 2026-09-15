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
use solid_facts_backend::fixture_authorization::{
    authorize_fixture_contract, read_fixture_contract_request,
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
/// The authorization is replaced and nothing else: the catalog's `import` block
/// is reused verbatim by `solid_facts_backend::fixture_authorization`, so this
/// swaps an obsolete policy-1 receipt for a test-scoped policy-2 one without
/// touching the resolution the fixture pins. That module is the single
/// implementation — `scripts/coverage.mjs` reaches the same code through
/// `solid-contract-authorize`, so a snapshot fixture and this corpus cannot
/// drift apart in what "authorized" means.
fn mint_and_analyze(
    fixture: &str,
    label: &str,
    reopen: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    mint_and_analyze_with_trust(fixture, label, reopen, true)
}

/// As above, with the choice of whether to hand the checker the trust
/// configuration. Withholding it is not a degenerate case — it is the control
/// that makes a fixed fixture signing key sound.
fn mint_and_analyze_with_trust(
    fixture: &str,
    label: &str,
    reopen: Option<&str>,
    supply_trust: bool,
) -> Result<Vec<serde_json::Value>, String> {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("caller guards on the producer");

    let scratch = temporary_directory(label);
    let project = scratch.join("consumer");
    copy_tree(&repository_root().join(fixture), &project);
    // The authorization canonicalizes the project it rebases paths against, and
    // the receipt binds the importer the consumer will compute. On macOS
    // `env::temp_dir()` is a symlink (`/var` -> `/private/var`), so an
    // uncanonicalized root here binds a path the consumer never derives.
    let project = fs::canonicalize(&project).unwrap();
    let request = read_fixture_contract_request(&project).map_err(|error| error.to_string())?;

    // `"<domain>-items"` strips the domain's positive operations instead of
    // reopening it: the claim stays closed, but over nothing. Reopening and
    // stripping are different questions — one removes the proof that an
    // enumeration is complete, the other removes the enumeration — and
    // conflating them is how a measurement of the first got reported as the
    // second.
    if let Some(domain) = reopen.and_then(|value| value.strip_suffix("-items")) {
        let mut contract: serde_json::Value =
            serde_json::from_slice(&fs::read(&request.document).unwrap()).unwrap();
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
        fs::write(&request.document, serde_json::to_vec(&contract).unwrap()).unwrap();
    } else if let Some(domain) = reopen {
        // Reopening one domain in the fixture's own document is how this file
        // asks whether *partial* closure is worth anything: every sibling
        // domain stays closed and usable.
        let mut contract: serde_json::Value =
            serde_json::from_slice(&fs::read(&request.document).unwrap()).unwrap();
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
        fs::write(&request.document, serde_json::to_vec(&contract).unwrap()).unwrap();
    }

    let authorization =
        authorize_fixture_contract(&project, &request).map_err(|error| error.to_string())?;

    // Out of band, exactly as an ordinary analysis requires, and *outside* the
    // project: the catalog never references trust bytes, so a project cannot
    // name its own issuer.
    let trust_path = scratch.join("fixture-trust.json");
    fs::write(&trust_path, &authorization.trust_configuration).unwrap();

    let mut arguments = vec![
        "--project".to_owned(),
        project.join("tsconfig.json").to_string_lossy().into_owned(),
        "--typefacts".to_owned(),
        typefacts,
        "--format".to_owned(),
        "json".to_owned(),
    ];
    if supply_trust {
        arguments.push("--receipt-trust-configuration".to_owned());
        arguments.push(trust_path.to_string_lossy().into_owned());
    }
    let output = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"))
        .args(&arguments)
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

/// The property that makes a fixed fixture signing key sound rather than a
/// forgery: a published, authenticated, correctly signed catalog still buys
/// nothing until a verifier is *separately* told to trust the issuer.
///
/// Without this, "the corpus can supply an accepted contract" would be
/// indistinguishable from "a directory can declare itself trusted", and every
/// snapshot downstream of an authorized fixture would be worthless. It is also
/// the reason `fixtures/reactive-ir/package-merged-props-consumer` can be
/// committed at all.
#[test]
fn an_authorized_catalog_is_refused_without_the_trust_configuration() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let refusal = mint_and_analyze_with_trust(FIXTURE, "unauthorized-trust", None, false)
        .expect_err("a policy-2 catalog with no trusted issuer must not be accepted");
    assert!(
        refusal.contains("authenticated issuer provenance"),
        "refused for the wrong reason: {refusal}"
    );
    // The control: the same tree, the same receipt, the trust supplied.
    mint_and_analyze_with_trust(FIXTURE, "authorized-trust", None, true)
        .expect("the same catalog is accepted once the issuer is trusted");
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

/// What a `callbacks` closure is worth to a consumer, measured by difference.
///
/// `callbacks` became proposable on 2026-09-12
/// (`phase21/2026-09-12-callbacks-census-scoping.md`), so the question its
/// admission raises is the one this project keeps having to answer for a
/// closure lever: does the closed domain change a *finding*, or only a count?
///
/// It is not one of SC9005's conjuncts — unlike `creates`, `reads`, `returns`
/// and `asyncBehavior` it is demand-scoped at the call site, through
/// `Indexes::unknown_contract_callback_export`, and the obligation is raised
/// only where a call actually hands over a potentially-callable argument
/// (`phase21/2026-09-10-sc9005-demand-scoping-design.md` § 1). So reopening it
/// must add an obligation exactly at such a call and nowhere else, which is
/// what makes the difference attributable.
///
/// Reported rather than asserted per fixture: the corpus that carries a
/// hand-closed `callbacks` domain is small, and pinning a count here would
/// break on every fixture added for an unrelated reason. The assertion is the
/// one that matters — at least one project moves, so the closure is not inert.
#[test]
fn a_callbacks_closure_discharges_the_call_site_obligation_that_reopening_restores() {
    if env::var("SOLID_TYPEFACTS_BIN").is_err() {
        return;
    }
    let incomplete = |rule: &str| rule.contains("package-contract-incomplete");
    let incomplete_count = |findings: &[serde_json::Value]| {
        findings
            .iter()
            .filter_map(|finding| finding["rule"].as_str())
            .filter(|rule| incomplete(rule))
            .count()
    };
    let mut moved = Vec::new();
    let mut indifferent = Vec::new();
    for (index, fixture) in catalog_bearing_fixtures().iter().enumerate() {
        let closed = mint_and_analyze(fixture, &format!("callbacks-closed-{index}"), None);
        let reopened = mint_and_analyze(
            fixture,
            &format!("callbacks-open-{index}"),
            Some("callbacks"),
        );
        let (Ok(closed), Ok(reopened)) = (closed, reopened) else {
            continue;
        };
        let (before, after) = (incomplete_count(&closed), incomplete_count(&reopened));
        if after > before {
            moved.push((fixture.clone(), before, after));
        } else {
            indifferent.push(fixture.clone());
        }
    }
    println!(
        "callbacks demand: {} of {} projects gain an obligation when the domain is reopened",
        moved.len(),
        moved.len() + indifferent.len()
    );
    for (fixture, before, after) in &moved {
        println!("  consumes    {fixture}  SC9005 {before} -> {after}");
    }
    for fixture in &indifferent {
        println!("  indifferent {fixture}");
    }
    assert!(
        !moved.is_empty(),
        "a closed callbacks domain must discharge at least one call-site obligation, or closing \
         it buys a consumer nothing: {indifferent:?}"
    );
}
