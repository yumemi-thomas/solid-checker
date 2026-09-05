//! Adversarial tests for the probe harness binding.
//!
//! Every case here attacks the binding with temporary files. Nothing in the
//! repository is mutated, and no test depends on this build carrying the
//! compiled-in pins: each one supplies its own `ProbeHarnessPin` so the
//! refusals are exercised on a development build too.

use super::*;
use crate::runtime_probe_wire::ReportedDependencyResolution;

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "solid-checker-probe-harness-test-{label}-{}-{}",
            std::process::id(),
            PRIVATE_HARNESS_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, relative: &str, bytes: &[u8]) -> PathBuf {
        let target = self.0.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("scratch parent");
        }
        fs::write(&target, bytes).expect("scratch write");
        target
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = remove_private_tree(&self.0);
    }
}

fn digest_of(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn pin(harness: &str, node: &str) -> ProbeHarnessPin {
    ProbeHarnessPin::new(harness, node).expect("test pin digests are canonical")
}

/// A harness root whose eight manifest members carry known bytes, plus the
/// build-provenance stamp the adapter cross-checks.
fn harness_root(scratch: &Scratch, worker_body: &str) -> String {
    for name in HARNESS_MANIFEST_FILES {
        let body = if name.ends_with("contract-probe-worker.mjs") {
            worker_body.to_owned()
        } else {
            format!("// {name}\n")
        };
        scratch.write(name, body.as_bytes());
    }
    let manifest =
        harness_source_manifest(scratch.path()).expect("a complete harness root has a manifest");
    let stamp = serde_json::json!({
        "format": 1,
        "sourceDigest": manifest.strip_prefix("sha256:").expect("canonical digest"),
        "toolchain": "test",
        "buildId": "test",
    });
    scratch.write(HARNESS_STAMP, format!("{stamp}\n").as_bytes());
    manifest
}

#[test]
fn a_development_build_without_pins_refuses_probe_authority() {
    // Whatever this build was configured with, the refusal shape is the same:
    // no pin means no probe authority, never a silently open gate.
    match ProbeHarnessPin::configured() {
        Err(ProbeHarnessError::PinUnavailable(message)) => {
            assert!(message.contains("configured probe"));
        }
        Err(other) => panic!("unexpected pin failure: {other}"),
        Ok(pin) => {
            assert!(pin.harness_manifest_sha256.starts_with("sha256:"));
            assert!(pin.node_executable_sha256.starts_with("sha256:"));
        }
    }
}

#[test]
fn a_node_binary_with_one_flipped_byte_is_refused() {
    let scratch = Scratch::new("flipped-node");
    let original = b"#!/bin/sh\nexit 0\n".to_vec();
    let node = scratch.write("node", &original);
    let pin = pin(&digest_of(b"harness"), &digest_of(&original));
    assert_eq!(
        verify_node_executable(&node, &pin).expect("the exact bytes verify"),
        digest_of(&original)
    );

    let mut flipped = original.clone();
    flipped[0] ^= 0x01;
    assert_eq!(flipped.len(), original.len());
    fs::write(&node, &flipped).expect("rewrite the node bytes");
    let error = verify_node_executable(&node, &pin).expect_err("one flipped byte must refuse");
    assert!(
        matches!(&error, ProbeHarnessError::NodeProvenance(message)
            if message.contains("do not match this build's configured digest")),
        "unexpected error: {error}"
    );
}

#[test]
#[cfg(unix)]
fn a_symlinked_node_binary_is_refused_even_when_its_target_matches() {
    let scratch = Scratch::new("symlinked-node");
    let bytes = b"#!/bin/sh\nexit 0\n".to_vec();
    let real = scratch.write("node", &bytes);
    let link = scratch.path().join("node-link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    let pin = pin(&digest_of(b"harness"), &digest_of(&bytes));
    // The target's bytes are exactly the pinned bytes; the *name* is still not
    // authority, because a symlink can be repointed after the pin was taken.
    let error = verify_node_executable(&link, &pin).expect_err("a symlink must refuse");
    assert!(
        matches!(&error, ProbeHarnessError::NodeProvenance(message)
            if message.contains("not a symlink")),
        "unexpected error: {error}"
    );
}

#[test]
fn a_tampered_harness_script_is_refused() {
    let scratch = Scratch::new("tampered-harness");
    let manifest = harness_root(&scratch, "// worker\n");
    let pin = pin(&manifest, &digest_of(b"node"));
    verify_harness_image(scratch.path(), &pin).expect("the exact image verifies");

    scratch.write(
        "packages/cli/scripts/contract-probe-worker.mjs",
        b"// worker\nprocess.exit(0);\n",
    );
    let error =
        verify_harness_image(scratch.path(), &pin).expect_err("a tampered script must refuse");
    assert!(
        matches!(&error, ProbeHarnessError::HarnessProvenance(message)
            if message.contains("does not match this build's configured source manifest")),
        "unexpected error: {error}"
    );
}

#[test]
fn a_stamp_that_disagrees_with_the_compiled_in_pin_is_refused() {
    let scratch = Scratch::new("stamp-mismatch");
    let manifest = harness_root(&scratch, "// worker\n");
    let pin = pin(&manifest, &digest_of(b"node"));
    scratch.write(
        HARNESS_STAMP,
        format!(
            "{}\n",
            serde_json::json!({
                "format": 1,
                "sourceDigest": "a".repeat(64),
                "toolchain": "test",
                "buildId": "test",
            })
        )
        .as_bytes(),
    );
    let error = verify_harness_image(scratch.path(), &pin).expect_err("a stale stamp must refuse");
    assert!(
        matches!(&error, ProbeHarnessError::HarnessProvenance(message)
            if message.contains("stamp does not match the configured pin")),
        "unexpected error: {error}"
    );
}

#[test]
fn the_bytes_that_verified_are_the_bytes_the_workspace_would_run() {
    // The image is read once. Swapping the file on disk after verification —
    // the window a second read would have opened — cannot change what a
    // private directory is populated with, because the bytes travel with the
    // verification rather than being fetched again from the path.
    let scratch = Scratch::new("harness-toctou");
    let manifest = harness_root(&scratch, "// worker\n");
    let pin = pin(&manifest, &digest_of(b"node"));
    let image = verify_harness_image(scratch.path(), &pin).expect("the exact image verifies");

    let member = "packages/cli/scripts/contract-probe-worker.mjs";
    let verified = image.member(member).expect("a manifest member").to_vec();
    scratch.write(member, b"// worker\nprocess.exit(0);\n");
    assert_eq!(
        image.member(member).expect("a manifest member"),
        verified.as_slice(),
        "a swap after verification must not reach the verified image"
    );
    assert_ne!(
        fs::read(scratch.path().join(member)).expect("read the swapped file"),
        verified,
        "the file on disk really was replaced, so the swap was a real one"
    );
    // And the image the workspace would write still hashes to the pin.
    let mut hash = Sha256::new();
    hash.update(b"solid-checker-probe-harness-source\0");
    hash.update(b"1");
    hash.update(b"\0");
    let mut names = HARNESS_MANIFEST_FILES;
    names.sort_unstable();
    for name in names {
        let contents = image.member(name).expect("a manifest member");
        hash.update(name.as_bytes());
        hash.update(b"\0");
        hash.update(contents.len().to_string().as_bytes());
        hash.update(b"\0");
        hash.update(contents);
        hash.update(b"\0");
    }
    assert_eq!(format!("sha256:{:x}", hash.finalize()), manifest);
}

#[test]
fn a_missing_harness_member_refuses_instead_of_hashing_what_is_there() {
    let scratch = Scratch::new("missing-harness-member");
    harness_root(&scratch, "// worker\n");
    fs::remove_file(
        scratch
            .path()
            .join("packages/cli/scripts/probe-contract.mjs"),
    )
    .expect("remove a manifest member");
    let error =
        harness_source_manifest(scratch.path()).expect_err("an incomplete image has no manifest");
    assert!(
        matches!(&error, ProbeHarnessError::HarnessProvenance(message)
            if message.contains("probe-contract.mjs")),
        "unexpected error: {error}"
    );
}

// ---------------------------------------------------------------------------
// Recipe corpus
// ---------------------------------------------------------------------------

fn corpus_manifest(module: &str) -> String {
    serde_json::json!({
        "format": RECIPE_CORPUS_FORMAT,
        "schemaVersion": RECIPE_CORPUS_SCHEMA_VERSION,
        "policy": {
            "repeatRuns": 2,
            "timeoutMillis": 5000,
            "maxMicrotaskTurns": 4,
            "maxMacrotaskTurns": 1,
            "maxEvents": 64,
        },
        "recipes": [{
            "claimId": format!("claim:v1:sha256:{}", "c".repeat(64)),
            "module": module,
            "importKind": "esm",
            "scenario": "operation",
            "expectedEvent": { "marker": "owner-created", "class": "cleanup" },
            "drain": [{ "kind": "microtasks", "maxTurns": 1 }],
            "coverageLimitations": ["one node build only"],
        }],
    })
    .to_string()
}

/// `RecipeCorpus::load` needs a plan only to refuse a corpus inside the
/// analyzed package. Every other property is exercised through the pieces the
/// loader is built from, so these tests need no live certification plan.
fn load_recipes(directory: &Path) -> Result<Vec<CorpusRecipe>, ProbeHarnessError> {
    let bytes =
        read_bounded_regular_file(&directory.join(RECIPE_CORPUS_MANIFEST), MAX_CORPUS_BYTES)?;
    let manifest: WireRecipeCorpus = serde_json::from_slice(&bytes).map_err(|error| {
        ProbeHarnessError::CorpusInvalid(format!("invalid probe recipe corpus: {error}"))
    })?;
    let mut recipes = Vec::new();
    for entry in manifest.recipes {
        let file_name = single_relative_component(&entry.module)?;
        let module_bytes = read_bounded_regular_file(&directory.join(&file_name), MAX_RECIPE_BYTES)
            .map_err(|error| ProbeHarnessError::RecipeProvenance(error.to_string()))?;
        let construction = Digest::parse(digest_of(&module_bytes)).expect("canonical digest");
        recipes.push(CorpusRecipe {
            claim_id: entry.claim_id,
            file_name,
            bytes: module_bytes,
            construction,
            import_kind: entry.import_kind,
            dependency_specifiers: entry.dependency_specifiers,
            scenario: entry.scenario.into(),
            expected_event: ProbeEventMatch {
                marker: entry.expected_event.marker,
                class: entry.expected_event.class,
                operation: None,
            },
            drain: entry.drain.into_iter().map(Into::into).collect(),
            coverage_limitations: entry.coverage_limitations,
        });
    }
    Ok(recipes)
}

#[test]
fn rust_derives_the_recipe_construction_digest_from_the_bytes_it_read() {
    let scratch = Scratch::new("recipe-construction");
    scratch.write(
        RECIPE_CORPUS_MANIFEST,
        corpus_manifest("clean.mjs").as_bytes(),
    );
    let body = b"export async function runProbeSession() {}\n";
    scratch.write("clean.mjs", body);
    let recipes = load_recipes(scratch.path()).expect("a well-formed corpus loads");
    assert_eq!(recipes.len(), 1);
    assert_eq!(recipes[0].construction.as_str(), digest_of(body));

    // Tampering the module changes the construction digest Rust binds, so the
    // recipe, plan, and session identities all move with it. A corpus cannot
    // present one identity and run other bytes.
    let tampered = b"export async function runProbeSession() { /* other */ }\n";
    scratch.write("clean.mjs", tampered);
    let retampered = load_recipes(scratch.path()).expect("the corpus still loads");
    assert_ne!(retampered[0].construction, recipes[0].construction);
    assert_eq!(retampered[0].construction.as_str(), digest_of(tampered));
}

#[test]
#[cfg(unix)]
fn a_symlinked_recipe_module_is_refused() {
    let scratch = Scratch::new("symlinked-recipe");
    scratch.write(
        RECIPE_CORPUS_MANIFEST,
        corpus_manifest("link.mjs").as_bytes(),
    );
    let real = scratch.write("real.mjs", b"export async function runProbeSession() {}\n");
    std::os::unix::fs::symlink(&real, scratch.path().join("link.mjs")).expect("symlink");
    let error = load_recipes(scratch.path()).expect_err("a symlinked module must refuse");
    assert!(
        matches!(&error, ProbeHarnessError::RecipeProvenance(message)
            if message.contains("regular non-symlink file")),
        "unexpected error: {error}"
    );
}

#[test]
fn a_recipe_module_outside_the_corpus_is_refused() {
    for escape in ["../outside.mjs", "nested/inside.mjs", "/absolute.mjs"] {
        let scratch = Scratch::new("escaping-recipe");
        scratch.write(RECIPE_CORPUS_MANIFEST, corpus_manifest(escape).as_bytes());
        let error = load_recipes(scratch.path()).expect_err("an escaping module path must refuse");
        assert!(
            matches!(&error, ProbeHarnessError::CorpusInvalid(message)
                if message.contains("one plain file name inside the corpus")),
            "unexpected error for {escape}: {error}"
        );
    }
}

// ---------------------------------------------------------------------------
// Path containment
// ---------------------------------------------------------------------------

#[test]
fn a_package_name_that_would_escape_node_modules_is_refused() {
    // The name arrives from a caller-supplied coordinate, and the upstream
    // coordinate validation admits a `..` segment. Joining it unchecked would
    // have written the private snapshot copy outside the private directory.
    for escape in [
        "../..",
        "..",
        "../elsewhere",
        "scope/name",
        "@scope/../name",
        "@scope",
        "@/name",
        "@scope/",
        ".hidden",
        "_private",
        "with space",
        "with/slash",
        "",
        // A second `node_modules` inside the private one, and the two CommonJS
        // global folders the census records as absent.
        "node_modules",
        ".node_modules",
        ".node_libraries",
        "@node_modules/name",
        // A trailing dot is a different name on a filesystem that strips it,
        // so the copy and the census would be about two different paths.
        "trailing.",
        "@scope/trailing.",
    ] {
        let error = safe_package_directory(escape)
            .expect_err("an unsafe package name must refuse before it is joined");
        assert!(
            matches!(&error, ProbeHarnessError::Configuration(message)
                if message.contains("plain npm package name")),
            "unexpected error for {escape:?}: {error}"
        );
    }
    assert_eq!(
        safe_package_directory("solid-js").expect("a plain name"),
        PathBuf::from("solid-js")
    );
    assert_eq!(
        safe_package_directory("@solid-primitives/marker").expect("a scoped name"),
        PathBuf::from("@solid-primitives").join("marker")
    );
}

#[test]
fn a_resolvable_node_modules_above_the_private_directory_refuses() {
    // A bare specifier the private copy does not answer walks *up*, so an
    // ancestor `node_modules` is bytes a probe could import that this
    // transaction never authenticated. The refusal comes before the launch;
    // one appearing *after* this check is caught by the watched census
    // instead (`an_ancestor_node_modules_appearing_during_the_run_refuses_the_gate`).
    let scratch = Scratch::new("ancestor-modules");
    let private = scratch.path().join("workspace");
    fs::create_dir_all(&private).expect("private directory");
    let node = scratch.write("bin/node", NODE_STAND_IN);
    // A machine that already has a `node_modules` somewhere above the
    // temporary directory refuses too, and that refusal is correct rather than
    // a test failure, so the clean direction is only asserted when the real
    // ancestry is in fact clean.
    let clean = refuse_resolvable_bare_specifier_sources(&private, &node).is_ok();

    let planted = scratch.path().join("node_modules");
    fs::create_dir(&planted).expect("an ancestor node_modules");
    let error = refuse_resolvable_bare_specifier_sources(&private, &node)
        .expect_err("a resolvable ancestor must refuse");
    let ProbeHarnessError::IsolationViolation(message) = &error else {
        panic!("unexpected error: {error}");
    };
    assert!(
        message.contains("resolvable from the private probe directory"),
        "unexpected message: {message}"
    );
    if clean {
        assert!(
            message.contains(&planted.display().to_string()),
            "the refusal must name the ancestor that caused it: {message}"
        );
    }
}

#[test]
fn the_commonjs_prefix_global_folder_is_refused_and_watched() {
    // `require`'s last resort is `globalPaths`, and its third entry is
    // `$PREFIX/lib/node` computed from `process.execPath` — not from the
    // environment, so `env_clear` does not remove it. It is a bare-specifier
    // source like an ancestor `node_modules`, so it is refused before a launch
    // and watched afterwards.
    let scratch = Scratch::new("prefix-lib-node");
    let private = scratch.path().join("workspace");
    fs::create_dir_all(&private).expect("private directory");
    let node = scratch.write("install/bin/node", NODE_STAND_IN);
    let candidate = scratch.path().join("install/lib/node");
    assert!(
        bare_specifier_sources(&private, &node)
            .iter()
            .any(|(label, path)| label.starts_with("node-prefix-lib-node:") && path == &candidate),
        "the Node install prefix's lib/node must be enumerated as a bare-specifier source"
    );

    let clean = refuse_resolvable_bare_specifier_sources(&private, &node).is_ok();
    fs::create_dir_all(&candidate).expect("plant a prefix global folder");
    let error = refuse_resolvable_bare_specifier_sources(&private, &node)
        .expect_err("a resolvable prefix global folder must refuse");
    let ProbeHarnessError::IsolationViolation(message) = &error else {
        panic!("unexpected error: {error}");
    };
    if clean {
        assert!(
            message.contains(&candidate.display().to_string()),
            "the refusal must name the folder that caused it: {message}"
        );
    }
}

#[test]
fn every_ancestor_package_json_is_watched_even_though_a_private_scope_shadows_it() {
    // `LOOKUP_PACKAGE_SCOPE` climbs to the first `package.json` above the
    // importer, and `PACKAGE_SELF_RESOLVE` runs *before* the `node_modules`
    // walk. The private scopes shadow every one of these, so this census is
    // belt to that brace — and it fails loudly if a future change drops a
    // scope.
    let scratch = Scratch::new("ancestor-package-json");
    let private = scratch.path().join("a/b/workspace");
    fs::create_dir_all(&private).expect("private directory");
    let scopes = ancestor_package_scopes(&private);
    for expected in [
        scratch.path().join("a/b/package.json"),
        scratch.path().join("a/package.json"),
        scratch.path().join("package.json"),
    ] {
        assert!(
            scopes
                .iter()
                .any(|(label, path)| label.starts_with("ancestor-package-json:")
                    && path == &expected),
            "{} must be watched",
            expected.display()
        );
    }
    // The private directory's *own* package.json is not an ancestor scope: the
    // scopes written inside it are what terminate the climb.
    assert!(
        !scopes
            .iter()
            .any(|(_, path)| path == &private.join("package.json")),
        "the private directory itself is not an ancestor of itself"
    );
}

// ---------------------------------------------------------------------------
// Write isolation
// ---------------------------------------------------------------------------

const NODE_STAND_IN: &[u8] = b"#!/bin/sh\nexit 0\n";

/// The one authenticated dependency `watched_workspace` places beside the
/// analyzed package's copy.
const WATCHED_DEPENDENCY: &str = "fixture-dependency";

/// A workspace shaped like the real one: the private `node_modules` with the
/// analyzed package's copy and one authenticated dependency copy beside it,
/// both `HOME`-relative global folders, one ancestor `node_modules` candidate,
/// and the Node executable with its pinned digest.
fn watched_workspace(scratch: &Scratch) -> PrivateProbeWorkspace {
    let modules = scratch.path().join("node_modules");
    let snapshot = modules.join("fixture");
    fs::create_dir_all(&snapshot).expect("snapshot copy");
    fs::write(snapshot.join("index.js"), b"export const value = 1;\n").expect("snapshot member");
    // The dependency copy, watched under its own label as well as by the
    // whole-tree `private-node-modules` census.
    let dependency = modules.join(WATCHED_DEPENDENCY);
    fs::create_dir_all(&dependency).expect("dependency copy");
    fs::write(dependency.join("index.js"), b"export const origin = 1;\n")
        .expect("dependency member");
    let node = scratch.write("node", NODE_STAND_IN);
    let ancestor_modules = scratch.path().join("workspace-parent/node_modules");
    let mut workspace = PrivateProbeWorkspace {
        directory: scratch.path().to_path_buf(),
        worker: scratch.path().join("harness/contract-probe-worker.mjs"),
        recipes: BTreeMap::new(),
        runtime_target: snapshot.join("index.js"),
        execution: None,
        dependency_roots: BTreeMap::from([(WATCHED_DEPENDENCY.to_owned(), dependency.clone())]),
        condition_flags: vec!["--conditions=import".into()],
        watched: vec![
            (
                "private-node-modules".into(),
                WatchedInput::Contents(modules),
            ),
            (
                format!("private-dependency:{WATCHED_DEPENDENCY}"),
                WatchedInput::Contents(dependency),
            ),
            (
                "home-node-modules".into(),
                WatchedInput::Contents(scratch.path().join(".node_modules")),
            ),
            (
                "home-node-libraries".into(),
                WatchedInput::Contents(scratch.path().join(".node_libraries")),
            ),
            (
                format!("ancestor-node-modules:{}", ancestor_modules.display()),
                WatchedInput::Contents(ancestor_modules),
            ),
            ("node-executable".into(), WatchedInput::Contents(node)),
        ],
        pinned: vec![("node-executable".into(), digest_of(NODE_STAND_IN))],
        before: BTreeMap::new(),
    };
    workspace.before = workspace.watch_digests().expect("initial digests");
    workspace
        .verify_pinned(&workspace.before)
        .expect("the baseline census asserts the pinned Node bytes");
    workspace
}

#[test]
fn node_bytes_swapped_after_the_pin_check_refuse_rather_than_being_censused() {
    // The pin verification, the `--version` query, the baseline census, and
    // every launch are separate reads of the same path. A census that recorded
    // whatever was on disk when the baseline was taken would faithfully record
    // — and then faithfully confirm — a binary substituted in between, so the
    // one watched path whose digest is known in advance is compared against
    // the *pin* instead.
    let scratch = Scratch::new("node-toctou");
    let workspace = watched_workspace(&scratch);
    workspace.verify_unchanged().expect("nothing changed yet");

    fs::write(scratch.path().join("node"), b"#!/bin/sh\nexec /bin/sh\n")
        .expect("swap the node bytes");
    let census = workspace.watch_digests().expect("a census still computes");
    for error in [
        workspace
            .verify_pinned(&census)
            .expect_err("a swapped Node binary must refuse the baseline census"),
        workspace
            .verify_unchanged()
            .expect_err("and every later census"),
    ] {
        assert!(
            matches!(&error, ProbeHarnessError::IsolationViolation(message)
                if message.contains("node-executable") && message.contains("this build pinned")),
            "unexpected error: {error}"
        );
    }
    // A census that no longer covers the pinned path is a refusal too, not a
    // vacuous pass over an empty list.
    let error = workspace
        .verify_pinned(&BTreeMap::new())
        .expect_err("an uncovered pinned path must refuse");
    assert!(
        matches!(&error, ProbeHarnessError::IsolationViolation(message)
            if message.contains("no longer covers")),
        "unexpected error: {error}"
    );
    std::mem::forget(workspace);
}

#[test]
fn an_ancestor_node_modules_appearing_during_the_run_refuses_the_gate() {
    // `refuse_resolvable_bare_specifier_sources` is a point-in-time check on a
    // shared tree — world-writable `/tmp` on Linux — so an ancestor
    // `node_modules` can be created after it passed and answer a bare
    // specifier the private copy does not. Each candidate it cleared is
    // therefore watched, recorded absent, and refuses on appearing.
    let scratch = Scratch::new("isolation-ancestor");
    let workspace = watched_workspace(&scratch);
    assert!(
        workspace.before.iter().any(
            |(label, digest)| label.starts_with("ancestor-node-modules:")
                && digest == ABSENT_WATCHED_PATH
        ),
        "an ancestor candidate that does not exist must be recorded as absent"
    );
    let planted = scratch
        .path()
        .join("workspace-parent/node_modules/smuggled");
    fs::create_dir_all(&planted).expect("plant an ancestor package");
    fs::write(planted.join("index.js"), b"export const value = 4;\n").expect("planted member");
    let error = workspace
        .verify_unchanged()
        .expect_err("an ancestor node_modules appearing must refuse");
    assert!(
        matches!(&error, ProbeHarnessError::IsolationViolation(message)
            if message.contains("ancestor-node-modules:")),
        "unexpected error: {error}"
    );
    std::mem::forget(workspace);
}

#[test]
fn an_altered_snapshot_copy_refuses_the_gate() {
    let scratch = Scratch::new("isolation-snapshot");
    let workspace = watched_workspace(&scratch);
    workspace.verify_unchanged().expect("nothing changed yet");

    fs::write(
        scratch.path().join("node_modules/fixture/index.js"),
        b"export const value = 2;\n",
    )
    .expect("simulate a probe write");
    let error = workspace
        .verify_unchanged()
        .expect_err("a changed snapshot copy must refuse");
    assert!(
        matches!(&error, ProbeHarnessError::IsolationViolation(message)
            if message.contains("private-node-modules changed")),
        "unexpected error: {error}"
    );
    // The workspace's Drop must not remove the caller's scratch tree twice.
    std::mem::forget(workspace);
}

#[test]
fn a_sibling_package_appearing_in_the_private_node_modules_refuses_the_gate() {
    // The census covers the whole private `node_modules`, not only the copied
    // package: a run that plants a sibling would otherwise satisfy a bare
    // specifier with bytes this transaction never authenticated, unwatched.
    let scratch = Scratch::new("isolation-sibling");
    let workspace = watched_workspace(&scratch);
    let sibling = scratch.path().join("node_modules/planted");
    fs::create_dir_all(&sibling).expect("plant a sibling package");
    fs::write(sibling.join("index.js"), b"export const value = 3;\n").expect("sibling member");
    assert!(matches!(
        workspace.verify_unchanged(),
        Err(ProbeHarnessError::IsolationViolation(_))
    ));
    std::mem::forget(workspace);
}

#[test]
fn an_altered_dependency_copy_refuses_the_gate_by_name() {
    // The authenticated dependency closure is watched exactly as the analyzed
    // package's copy is, and under its own label: a dependency file changing
    // between launches or before the final census refuses the gate, and the
    // refusal says *which* dependency rather than reporting an anonymous change
    // somewhere under `node_modules`.
    let scratch = Scratch::new("isolation-dependency");
    let workspace = watched_workspace(&scratch);
    workspace.verify_unchanged().expect("nothing changed yet");
    assert!(
        workspace
            .before
            .contains_key(&format!("private-dependency:{WATCHED_DEPENDENCY}")),
        "each dependency copy is censused before the first launch: {:?}",
        workspace.before.keys().collect::<Vec<_>>()
    );

    fs::write(
        scratch
            .path()
            .join(format!("node_modules/{WATCHED_DEPENDENCY}/index.js")),
        b"export const origin = 2;\n",
    )
    .expect("simulate a probe write into the dependency copy");
    let error = workspace
        .verify_unchanged()
        .expect_err("a changed dependency copy must refuse");
    assert!(
        matches!(&error, ProbeHarnessError::IsolationViolation(message)
            if message.contains(&format!("private-dependency:{WATCHED_DEPENDENCY} changed"))),
        "unexpected error: {error}"
    );
    // A new file inside the dependency copy is a change too — a run that
    // planted a module there would otherwise answer a deep specifier of that
    // dependency with bytes nothing authenticated.
    let scratch = Scratch::new("isolation-dependency-new-file");
    let workspace = watched_workspace(&scratch);
    fs::write(
        scratch
            .path()
            .join(format!("node_modules/{WATCHED_DEPENDENCY}/extra.js")),
        b"// added by a probe\n",
    )
    .expect("plant a module inside the dependency copy");
    assert!(matches!(
        workspace.verify_unchanged(),
        Err(ProbeHarnessError::IsolationViolation(_))
    ));
    std::mem::forget(workspace);
}

#[test]
fn a_declared_dependency_resolution_outside_the_authenticated_copy_refuses_the_gate() {
    // The echoed answer for a declared dependency specifier has to name a file
    // *inside* that dependency's authenticated private copy. Four refusals, and
    // the failure direction of each is a refused gate rather than a probe
    // against bytes this transaction never authenticated: a specifier no copy
    // was placed for, an answer that resolved nowhere, an answer outside the
    // copy, and a frame whose entry count or order is not the one asked for.
    let scratch = Scratch::new("declared-dependency");
    let inside = scratch.path().join("node_modules/dep");
    fs::create_dir_all(&inside).expect("dependency copy");
    let member = inside.join("index.mjs");
    fs::write(&member, b"export const origin = 1;\n").expect("dependency member");
    let elsewhere = scratch.write("elsewhere/index.mjs", b"export const origin = 2;\n");
    let roots = BTreeMap::from([("dep".to_owned(), inside.clone())]);
    let declared = ["dep".to_owned()];
    let url = |path: &Path| format!("file://{}", path.display());
    let dependency = |specifier: &str, esm: &str| ReportedDependencyResolution {
        specifier: specifier.to_owned(),
        esm: esm.to_owned(),
        require: esm.to_owned(),
    };
    let report = |entries: Vec<ReportedDependencyResolution>| ReportedResolution {
        specifier: "pkg".to_owned(),
        import_kind: "esm".to_owned(),
        esm: url(&member),
        require: member.display().to_string(),
        dependencies: entries,
    };

    // The accepting direction first, so none of the refusals below is vacuous.
    verify_reported_dependency_resolutions(
        &report(vec![dependency("dep", &url(&member))]),
        ProbeImportKind::Esm,
        &declared,
        &roots,
    )
    .expect("an answer inside the authenticated copy must be accepted");

    for (entries, fragment) in [
        (
            Vec::new(),
            "declared dependency specifier(s) and it reported",
        ),
        (
            vec![
                dependency("dep", &url(&member)),
                dependency("dep", &url(&member)),
            ],
            "declared dependency specifier(s) and it reported",
        ),
        (
            vec![dependency("other", &url(&member))],
            "where this launch asked for",
        ),
        (
            vec![dependency("dep", "unresolved:ERR_MODULE_NOT_FOUND")],
            "not a local file",
        ),
        (
            vec![dependency("dep", &url(&elsewhere))],
            "not inside the authenticated private copy",
        ),
    ] {
        let error = verify_reported_dependency_resolutions(
            &report(entries),
            ProbeImportKind::Esm,
            &declared,
            &roots,
        )
        .expect_err("an unusable dependency resolution must refuse");
        assert!(
            matches!(&error, ProbeHarnessError::ConditionMismatch(message)
                if message.contains(fragment)),
            "unexpected error for {fragment:?}: {error}"
        );
    }

    // A declared specifier with no placed copy at all: the workspace never
    // authenticated it, so there is nothing to compare against and the gate
    // refuses by name rather than accepting whatever the worker resolved.
    let error = verify_reported_dependency_resolutions(
        &report(vec![dependency("dep", &url(&member))]),
        ProbeImportKind::Esm,
        &declared,
        &BTreeMap::new(),
    )
    .expect_err("a declared specifier with no authenticated copy must refuse");
    assert!(
        matches!(&error, ProbeHarnessError::ConditionMismatch(message)
            if message.contains("placed no authenticated copy")),
        "unexpected error: {error}"
    );
}

#[test]
fn a_commonjs_global_folder_appearing_under_home_refuses_the_gate() {
    // `HOME` is the private directory, which makes `<private>/.node_modules`
    // and `<private>/.node_libraries` CommonJS global folders for the worker.
    // Neither exists when the census is taken, so the watched digest for each
    // is the absent marker and either appearing is a change.
    for folder in [".node_modules", ".node_libraries"] {
        let scratch = Scratch::new("isolation-global-folder");
        let workspace = watched_workspace(&scratch);
        assert!(
            workspace
                .before
                .iter()
                .any(|(label, digest)| label.starts_with("home-node-")
                    && digest == ABSENT_WATCHED_PATH),
            "a global folder that does not exist must be recorded as absent"
        );
        let planted = scratch.path().join(folder);
        fs::create_dir(&planted).expect("plant a global folder");
        fs::write(planted.join("smuggled.js"), b"module.exports = 1;\n").expect("planted member");
        let error = workspace
            .verify_unchanged()
            .expect_err("a global folder appearing must refuse");
        assert!(
            matches!(&error, ProbeHarnessError::IsolationViolation(message)
                if message.contains("home-node-")),
            "unexpected error for {folder}: {error}"
        );
        std::mem::forget(workspace);
    }
}

#[test]
fn a_new_file_in_a_watched_tree_refuses_the_gate() {
    let scratch = Scratch::new("isolation-new-file");
    let workspace = watched_workspace(&scratch);
    fs::write(
        scratch.path().join("node_modules/fixture/extra.js"),
        b"// added by a probe\n",
    )
    .expect("simulate a probe write");
    assert!(matches!(
        workspace.verify_unchanged(),
        Err(ProbeHarnessError::IsolationViolation(_))
    ));
    std::mem::forget(workspace);
}

#[test]
#[cfg(unix)]
fn a_watched_file_replaced_by_a_symlink_refuses_the_gate() {
    let scratch = Scratch::new("isolation-symlink");
    let workspace = watched_workspace(&scratch);
    let member = scratch.path().join("node_modules/fixture/index.js");
    fs::remove_file(&member).expect("remove the copy");
    std::os::unix::fs::symlink(scratch.path().join("node"), &member).expect("symlink");
    let error = workspace
        .verify_unchanged()
        .expect_err("a symlinked watched member must refuse");
    assert!(
        matches!(&error, ProbeHarnessError::IsolationViolation(message)
            if message.contains("no longer a regular file")),
        "unexpected error: {error}"
    );
    std::mem::forget(workspace);
}

// ---------------------------------------------------------------------------
// Startup frame
// ---------------------------------------------------------------------------

/// A startup frame with every field overridable, so each check can be isolated.
fn startup_frame(
    format: &str,
    protocol: &str,
    nonce: &str,
    version: &str,
    platform: &str,
    architecture: &str,
) -> String {
    serde_json::json!({
        "format": format,
        "protocol": protocol,
        "nonce": nonce,
        "nodeVersion": version,
        "platform": platform,
        "architecture": architecture,
    })
    .to_string()
}

#[test]
fn the_startup_frame_must_echo_protocol_nonce_version_platform_and_architecture() {
    let platform = node_platform_name();
    let architecture = node_architecture_name();
    let frame = |protocol: &str, nonce: &str, version: &str| {
        startup_frame(
            STARTUP_FORMAT,
            protocol,
            nonce,
            version,
            platform,
            architecture,
        )
    };
    verify_startup_frame(
        &frame(PROBE_WORKER_PROTOCOL, "nonce-1", "v24.0.0"),
        "nonce-1",
        "v24.0.0",
    )
    .expect("an exact echo is accepted");

    for (line, expected) in [
        (frame("other-protocol", "nonce-1", "v24.0.0"), "protocol"),
        (frame(PROBE_WORKER_PROTOCOL, "nonce-2", "v24.0.0"), "nonce"),
        (frame(PROBE_WORKER_PROTOCOL, "nonce-1", "v22.0.0"), "Node"),
        // The verifier's own `std::env::consts` are what the recorded
        // environment says, so the worker has to confirm the platform rather
        // than have it assumed of it.
        (
            startup_frame(
                STARTUP_FORMAT,
                PROBE_WORKER_PROTOCOL,
                "nonce-1",
                "v24.0.0",
                "some-other-platform",
                architecture,
            ),
            "platform",
        ),
        (
            startup_frame(
                STARTUP_FORMAT,
                PROBE_WORKER_PROTOCOL,
                "nonce-1",
                "v24.0.0",
                platform,
                "some-other-architecture",
            ),
            "architecture",
        ),
    ] {
        let error = verify_startup_frame(&line, "nonce-1", "v24.0.0")
            .expect_err("a mismatched frame must refuse");
        assert!(
            matches!(&error, ProbeHarnessError::Protocol(message) if message.contains(expected)),
            "unexpected error: {error}"
        );
    }

    assert!(matches!(
        verify_startup_frame("{}", "nonce-1", "v24.0.0"),
        Err(ProbeHarnessError::Protocol(_))
    ));
    assert!(matches!(
        verify_startup_frame(
            &startup_frame(
                "something-else",
                PROBE_WORKER_PROTOCOL,
                "nonce-1",
                "v24.0.0",
                platform,
                architecture,
            ),
            "nonce-1",
            "v24.0.0",
        ),
        Err(ProbeHarnessError::Protocol(_))
    ));
}

#[test]
fn the_startup_frame_platform_names_are_nodes_own_spelling() {
    // The mapping exists because the two runtimes spell the same platform
    // differently; an unmapped name falls through to itself, which refuses
    // unless Node agrees, and that is the fail-closed direction.
    match std::env::consts::OS {
        "macos" => assert_eq!(node_platform_name(), "darwin"),
        "windows" => assert_eq!(node_platform_name(), "win32"),
        other => assert_eq!(node_platform_name(), other),
    }
    match std::env::consts::ARCH {
        "x86_64" => assert_eq!(node_architecture_name(), "x64"),
        "aarch64" => assert_eq!(node_architecture_name(), "arm64"),
        other => assert_eq!(node_architecture_name(), other),
    }
}

#[test]
fn a_third_report_frame_is_refused_rather_than_chosen_between() {
    // Taking the first or the last frame would let a worker that wrote several
    // decide which one the verifier believes. The reader stops at the protocol
    // budget and reports the excess instead.
    let scratch = Scratch::new("report-frames");
    let path = scratch.write(
        "report",
        b"{\"one\":1}\n{\"two\":2}\n{\"three\":3}\n{\"four\":4}\n",
    );
    let receiver = spawn_report_reader(File::open(&path).expect("open the report"));
    for expected in ["{\"one\":1}", "{\"two\":2}"] {
        let line = receiver
            .recv_timeout(STARTUP_BUDGET)
            .expect("a queued frame")
            .expect("a readable frame");
        assert_eq!(line.trim(), expected);
    }
    let error = receiver
        .recv_timeout(STARTUP_BUDGET)
        .expect("the reader reports the excess")
        .expect_err("a third frame must be refused");
    assert!(
        error.to_string().contains("report frames were written"),
        "unexpected error: {error}"
    );
}

#[test]
fn a_report_that_exceeds_its_byte_budget_is_refused() {
    let scratch = Scratch::new("report-bytes");
    let oversized = vec![b'x'; MAX_REPORT_BYTES + 16];
    let path = scratch.write("report", &oversized);
    let receiver = spawn_report_reader(File::open(&path).expect("open the report"));
    let error = receiver
        .recv_timeout(STARTUP_BUDGET)
        .expect("the reader answers")
        .expect_err("an oversized report must be refused");
    assert!(
        error.to_string().contains("byte budget"),
        "unexpected error: {error}"
    );
}

#[test]
fn a_build_that_must_carry_probe_pins_carries_them() {
    // `option_env!` is read at *compile* time, so a test binary built without
    // the pins turns every production-path tracer into a silent early return
    // and takes the whole binding out of the gate with it — which is exactly
    // what `scripts/verify.sh` used to do. It now exports the pins and sets
    // this variable, so their absence is a loud failure there instead of a
    // green run that proved nothing.
    if std::env::var("SOLID_CHECKER_EXPECT_PROBE_PINS").as_deref() != Ok("1") {
        return;
    }
    assert!(
        configured_pin_digests().is_some(),
        "SOLID_CHECKER_EXPECT_PROBE_PINS=1, but this binary was compiled without \
         SOLID_CHECKER_PROBE_HARNESS_SHA256/SOLID_CHECKER_PROBE_NODE_SHA256: every \
         production-path probe assertion would return early. Build with the Makefile's \
         CERTIFICATION_ENV (make test-rust, make build-checker-debug), not bare cargo."
    );
}

#[test]
fn the_sandbox_policy_digest_names_what_is_not_denied() {
    // The field vector, verbatim. Asserting only that the digest is stable and
    // starts with `sha256:` was vacuous: dropping a `resolution:` field,
    // renaming one, or bumping `scheme-version` without changing anything the
    // receipt reader can see would all have passed. The literal below is the
    // second copy on purpose — a change has to be made twice, and the diff
    // says which field moved.
    const EXPECTED: [&str; 46] = [
        "scheme-version:10",
        "profile:inert-or-import-free-or-relative-ts-graph-esm,explicit-controlled-consumer,ordinary-acceptance-refused",
        "transform:pinned-node-strip-only,parser-runtime-token-preservation,all-derived-outputs-compared,watched-derived-graph",
        "resolution:profile-hook-exact-source-url-and-authenticated-relative-edge-map,unmapped-profile-imports-refused",
        "enforcement:detect-and-refuse",
        "private-directory-mode:0700",
        "snapshot:private-copy-per-transaction",
        "snapshot:analyzed-package-plus-authenticated-dependency-closure",
        "snapshot:dependency-closure-from-transaction-authenticated-snapshots-only",
        "snapshot:one-version-per-dependency-name-or-refuse",
        "snapshot:unauthenticated-package-dependency-refuses-by-name",
        "snapshot:dependency-materialization-manifest-bound-to-probe-root",
        "cwd:private-directory",
        "environment:allowlisted-not-inherited",
        "argv:worker-path-plus-requested-conditions-only",
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
    assert_eq!(
        SANDBOX_POLICY_FIELDS, EXPECTED,
        "the receipt-visible sandbox policy changed: update the disposition table in \
         docs/adr/0006-probe-harness-binding.md, bump scheme-version, and copy the new vector here"
    );
    // The scheme it commits to says outright that writes and network are
    // detected rather than denied, so a receipt can never be read as claiming
    // OS-level isolation.
    for expected in [
        "enforcement:detect-and-refuse",
        "network:not-denied",
        "filesystem-writes:not-denied",
    ] {
        assert!(
            SANDBOX_POLICY_FIELDS.contains(&expected),
            "{expected} must stay on the record"
        );
    }
    let digest = sandbox_policy_digest();
    assert_eq!(digest, sandbox_policy_digest());
    assert_eq!(digest, root("probe-sandbox-policy", EXPECTED));
    assert_ne!(
        digest,
        root(
            "probe-sandbox-policy",
            ["scheme-version:1", "enforcement:os-level-denial"]
        )
    );
}

#[test]
fn a_repeated_watched_label_refuses_rather_than_being_merged() {
    // The census is consulted *by label* — `verify_pinned` looks the pinned
    // Node digest up — so two entries under one name would leave one
    // unreadable and the other silently authoritative, and `verify_unchanged`
    // would compare two censuses that agree in length while disagreeing about
    // which path each entry is. Nothing in `create` builds a duplicate today;
    // this is the invariant that makes a future one a refusal rather than a
    // silent narrowing of the census.
    let scratch = Scratch::new("duplicate-label");
    let workspace = watched_workspace(&scratch);
    let mut duplicated = PrivateProbeWorkspace {
        directory: workspace.directory.clone(),
        worker: workspace.worker.clone(),
        recipes: BTreeMap::new(),
        runtime_target: workspace.runtime_target.clone(),
        execution: None,
        dependency_roots: BTreeMap::new(),
        condition_flags: workspace.condition_flags.clone(),
        watched: workspace.watched.clone(),
        pinned: workspace.pinned.clone(),
        before: BTreeMap::new(),
    };
    std::mem::forget(workspace);
    // The same label twice, pointing at two different paths — the shape that
    // would make one of them unreadable.
    duplicated.watched.push((
        "private-node-modules".into(),
        WatchedInput::Contents(scratch.path().join("node_modules/fixture")),
    ));
    let error = duplicated
        .watch_digests()
        .expect_err("a repeated census label must refuse");
    assert!(
        matches!(&error, ProbeHarnessError::IsolationViolation(message)
            if message.contains("names private-node-modules twice")),
        "unexpected error: {error}"
    );
    std::mem::forget(duplicated);
}

// ---------------------------------------------------------------------------
// Module resolution, against Node's own algorithm
// ---------------------------------------------------------------------------

/// The Node executable the resolution tests below run.
///
/// These are the only tests here that need a real interpreter: the property
/// under test is what *Node* resolves, and asserting it against a
/// reimplementation of the algorithm would assert the reimplementation.
///
/// Under `SOLID_CHECKER_EXPECT_PROBE_PINS=1` — which `scripts/verify.sh` and
/// `make test-rust` set when a Node runtime exists — an absent one is a loud
/// failure rather than a silent skip.
fn resolution_node() -> Option<PathBuf> {
    let resolved = std::env::var_os("SOLID_CHECKER_PROBE_NODE")
        .or_else(|| std::env::var_os("PROBE_NODE"))
        .map(PathBuf::from)
        .or_else(|| {
            let output = Command::new("/bin/sh")
                .arg("-c")
                .arg("command -v node")
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_owned()))
        })
        .and_then(|candidate| fs::canonicalize(candidate).ok());
    assert!(
        resolved.is_some()
            || std::env::var("SOLID_CHECKER_EXPECT_PROBE_PINS").as_deref() != Ok("1"),
        "SOLID_CHECKER_EXPECT_PROBE_PINS=1, but no Node executable could be resolved: the \
         module-resolution containment tests would skip and the escape they pin would leave \
         the gate silently"
    );
    resolved
}

#[test]
fn probe_type_erasure_reflection_does_not_establish_consumer_compatibility() {
    let pin = ProbeHarnessPin::configured();
    assert!(
        pin.is_ok() || std::env::var("SOLID_CHECKER_EXPECT_PROBE_PINS").as_deref() != Ok("1"),
        "the erasure regression must run through the Makefile with compiled pins"
    );
    let Ok(pin) = pin else { return };
    let Some(node) = resolution_node() else {
        return;
    };
    verify_node_executable(&node, &pin).expect("use this build's pinned Node, not POC pins");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let output = Command::new(node)
        .arg(repository.join("fixtures/package-contracts/restricted-type-erasure/compare.mjs"))
        .arg(repository.join("packages/cli/node_modules/typescript/lib/typescript.js"))
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .expect("launch the fixed reflection regression, not a certification worker");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "strip=false;typescript=true;derived-contradiction=true;enum=refused;tsx=refused\n"
    );
}

const RESOLUTION_PACKAGE: &str = "fixture-probe-package";

/// What the resolver module reports for each of the three resolution steps that
/// reach outside the private tree.
struct ResolutionOutcome {
    esm: String,
    package_imports: String,
    common_js: String,
}

/// Runs `recipes/resolve.mjs` inside the private tree with the launch's own
/// environment allowlist and returns what it resolved.
fn resolve_from_private_recipes(node: &Path, private: &Path) -> ResolutionOutcome {
    let mut command = Command::new(node);
    command
        .arg(private.join("recipes/resolve.mjs"))
        .current_dir(private);
    command.env_clear();
    // The launch's allowlist, so this test resolves under the environment the
    // harness actually gives a worker.
    command.env("HOME", private);
    command.env("TMPDIR", private);
    command.env("LANG", "C");
    command.env("LC_ALL", "C");
    command.env("NODE_OPTIONS", "");
    let output = command
        .output()
        .expect("run the resolver under the pinned Node");
    assert!(
        output.status.success(),
        "the resolver exited {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(name))
            .unwrap_or_else(|| panic!("the resolver reported no {name} line in {text:?}"))
            .to_owned()
    };
    ResolutionOutcome {
        esm: field("esm:"),
        package_imports: field("imports:"),
        common_js: field("cjs:"),
    }
}

/// A private tree with the production layout, a private copy of a package, and
/// an **ancestor package.json naming that same package** — the escape round
/// three found.
fn resolution_workspace(scratch: &Scratch) -> PathBuf {
    // The ancestor self-reference. `<tmpdir>` is world-writable on Linux, so
    // this is a file an unrelated user can plant.
    scratch.write(
        "package.json",
        format!(
            "{{\"name\":\"{RESOLUTION_PACKAGE}\",\"version\":\"9.9.9\",\"exports\":{{\".\":\
             {{\"import\":\"./stub.mjs\",\"require\":\"./stub.cjs\"}}}},\"imports\":\
             {{\"#stub\":\"./stub.mjs\"}}}}\n"
        )
        .as_bytes(),
    );
    scratch.write("stub.mjs", b"export const origin = \"ancestor-stub\";\n");
    scratch.write(
        "stub.cjs",
        b"module.exports = { origin: \"ancestor-stub\" };\n",
    );

    let private = scratch.path().join("workspace");
    fs::create_dir(&private).expect("private directory");
    let layout = create_private_layout(&private).expect("the production private layout");

    let package = layout.modules.join(RESOLUTION_PACKAGE);
    fs::create_dir_all(&package).expect("private snapshot copy");
    fs::write(
        package.join("package.json"),
        format!(
            "{{\"name\":\"{RESOLUTION_PACKAGE}\",\"version\":\"1.0.0\",\"exports\":{{\".\":\
             {{\"import\":\"./index.mjs\",\"require\":\"./index.cjs\"}}}}}}\n"
        ),
    )
    .expect("private manifest");
    fs::write(
        package.join("index.mjs"),
        b"export const origin = \"private-copy\";\n",
    )
    .expect("private ESM member");
    fs::write(
        package.join("index.cjs"),
        b"module.exports = { origin: \"private-copy\" };\n",
    )
    .expect("private CommonJS member");

    // A stand-in for a recipe: it imports its package exactly as every recipe
    // in `fixtures/package-contracts/closed-domain-probe-gate/probe-recipes`
    // does, by bare specifier, and reports where that landed.
    fs::write(
        layout.recipes.join("resolve.mjs"),
        format!(
            "const esm = await import(\"{RESOLUTION_PACKAGE}\");\n\
             console.log(`esm:${{esm.origin}}`);\n\
             try {{\n\
             \x20 const scoped = await import(\"#stub\");\n\
             \x20 console.log(`imports:${{scoped.origin}}`);\n\
             }} catch {{\n\
             \x20 console.log(\"imports:refused\");\n\
             }}\n\
             const {{ createRequire }} = await import(\"node:module\");\n\
             console.log(`cjs:${{createRequire(import.meta.url)(\"{RESOLUTION_PACKAGE}\").origin}}`);\n"
        ),
    )
    .expect("resolver module");
    private
}

#[test]
fn an_ancestor_package_self_reference_cannot_answer_a_private_bare_specifier() {
    // The round-three escape, and the one that was a false *pass* rather than a
    // refusal. `PACKAGE_SELF_RESOLVE` runs before the `node_modules` walk, and
    // it starts at `LOOKUP_PACKAGE_SCOPE`, which climbs from the recipe to the
    // first `package.json` above it. Without a nearer one that is
    // `<tmpdir>/package.json`; a `{"name": "<the analyzed package>",
    // "exports": …}` planted there wins over the private snapshot copy and the
    // recipe observes a conforming stub, so the veto passes and the closure
    // certifies on a value the real package never shipped.
    //
    // `PACKAGE_IMPORTS_RESOLVE` (a `#specifier`) and the CommonJS `trySelf`
    // start from the same lookup, so all three are asserted.
    let Some(node) = resolution_node() else {
        eprintln!("module-resolution containment test skipped: no node runtime");
        return;
    };
    let scratch = Scratch::new("self-reference");
    let private = resolution_workspace(&scratch);

    let contained = resolve_from_private_recipes(&node, &private);
    assert_eq!(
        contained.esm, "private-copy",
        "a bare import must resolve to the authenticated private copy"
    );
    assert_eq!(
        contained.common_js, "private-copy",
        "a CommonJS require must resolve to the authenticated private copy"
    );
    assert_eq!(
        contained.package_imports, "refused",
        "a `#` specifier must not reach an ancestor scope's `imports` map"
    );

    // The other half, and the reason this test is not decoration: removing the
    // private package scope hands every one of those three to the ancestor. A
    // change that drops the scope therefore fails here rather than certifying a
    // stub.
    fs::remove_file(private.join("recipes/package.json")).expect("remove the private scope");
    let escaped = resolve_from_private_recipes(&node, &private);
    assert_eq!(
        escaped.esm, "ancestor-stub",
        "without the private package scope the ancestor self-reference must win — if it does \
         not, this test has stopped pinning the containment"
    );
    assert_eq!(escaped.common_js, "ancestor-stub");
    assert_eq!(escaped.package_imports, "ancestor-stub");
}

const RESOLUTION_DEPENDENCY: &str = "fixture-probe-dependency";

/// The production layout with the analyzed package's copy *and* one
/// authenticated dependency copy beside it, where the package's own top level
/// imports that dependency by bare specifier.
///
/// This is the shape a real consumer row has: a recipe cannot import the
/// package under test at all unless the package's own
/// `import "<dependency>"` resolves as soon as the module evaluates.
fn dependency_resolution_workspace(scratch: &Scratch) -> (PathBuf, PathBuf) {
    let private = scratch.path().join("workspace");
    fs::create_dir(&private).expect("private directory");
    let layout = create_private_layout(&private).expect("the production private layout");

    let package = layout.modules.join(RESOLUTION_PACKAGE);
    fs::create_dir_all(&package).expect("private snapshot copy");
    fs::write(
        package.join("package.json"),
        format!(
            "{{\"name\":\"{RESOLUTION_PACKAGE}\",\"version\":\"1.0.0\",\"exports\":{{\".\":\
             \"./index.mjs\"}}}}\n"
        ),
    )
    .expect("private manifest");
    // The package's own bare dependency import, at module top level: it runs
    // the moment a recipe imports this package.
    fs::write(
        package.join("index.mjs"),
        format!(
            "import {{ origin }} from \"{RESOLUTION_DEPENDENCY}\";\n\
             export const dependencyOrigin = origin;\n"
        ),
    )
    .expect("private ESM member");

    let dependency = layout.modules.join(RESOLUTION_DEPENDENCY);
    fs::create_dir_all(&dependency).expect("private dependency copy");
    // Its *own* authenticated manifest, with its own `exports` map: this module
    // writes no package scope into a dependency copy, because that map is what
    // answers its specifiers.
    fs::write(
        dependency.join("package.json"),
        format!(
            "{{\"name\":\"{RESOLUTION_DEPENDENCY}\",\"version\":\"1.0.0\",\"exports\":{{\".\":\
             \"./main.mjs\"}}}}\n"
        ),
    )
    .expect("dependency manifest");
    fs::write(
        dependency.join("main.mjs"),
        b"export const origin = \"private-dependency\";\n",
    )
    .expect("dependency member");

    fs::write(
        layout.recipes.join("resolve.mjs"),
        format!(
            "const pkg = await import(\"{RESOLUTION_PACKAGE}\");\n\
             console.log(`esm:${{pkg.dependencyOrigin}}`);\n\
             console.log(`imports:${{import.meta.resolve(\"{RESOLUTION_DEPENDENCY}\")}}`);\n\
             const {{ createRequire }} = await import(\"node:module\");\n\
             console.log(`cjs:${{createRequire(import.meta.url).resolve(\"{RESOLUTION_DEPENDENCY}\")}}`);\n"
        ),
    )
    .expect("resolver module");
    (private, dependency)
}

#[test]
fn a_recipe_imports_the_package_and_its_dependency_resolves_inside_the_private_copy() {
    // The whole point of the dependency closure, against a real interpreter:
    // a recipe imports the package under test by bare specifier, the package's
    // own top-level `import "<dependency>"` runs, and it lands inside the
    // authenticated private copy — which is exactly what
    // `verify_reported_dependency_resolutions` requires of the echo a launch
    // reports.
    let Some(node) = resolution_node() else {
        eprintln!("dependency-closure resolution test skipped: no node runtime");
        return;
    };
    let scratch = Scratch::new("dependency-closure");
    let (private, dependency_root) = dependency_resolution_workspace(&scratch);

    let contained = resolve_from_private_recipes(&node, &private);
    assert_eq!(
        contained.esm, "private-dependency",
        "the package under test must be importable, which needs its own dependency import to \
         resolve inside the private workspace"
    );
    for (label, observed) in [
        ("import.meta.resolve", contained.package_imports.as_str()),
        ("createRequire", contained.common_js.as_str()),
    ] {
        let resolved = match observed.strip_prefix("file://") {
            Some(_) => decode_file_url(observed),
            None => Some(PathBuf::from(observed)),
        }
        .unwrap_or_else(|| panic!("{label} reported {observed:?}, which is not a local path"));
        assert!(
            path_is_inside(&resolved, &dependency_root),
            "{label} resolved the dependency to {observed:?}, which is not inside the \
             authenticated copy at {}",
            dependency_root.display()
        );
    }

    // The other half, so the test cannot go vacuous: remove the dependency copy
    // and the package is no longer importable at all. That is the pre-change
    // behaviour — `ERR_MODULE_NOT_FOUND`, a failed run, a refused gate — and it
    // is what made every consumer closure candidate unprobeable.
    remove_private_tree(&dependency_root).expect("remove the dependency copy");
    let mut command = Command::new(&node);
    command
        .arg(private.join("recipes/resolve.mjs"))
        .current_dir(&private)
        .env_clear()
        .env("HOME", &private)
        .env("TMPDIR", &private)
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("NODE_OPTIONS", "");
    let output = command.output().expect("run the resolver");
    assert!(
        !output.status.success()
            && String::from_utf8_lossy(&output.stderr).contains("ERR_MODULE_NOT_FOUND"),
        "without the dependency copy the package must not be importable — if it is, this test \
         has stopped pinning what the closure buys: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_private_package_scope_can_answer_nothing() {
    // The manifest is the *nearest* scope for the harness image and every
    // recipe, so what it does not carry is the whole point: `exports` and
    // `main` are what let a scope answer a self-reference, and `imports` is
    // what lets it answer a `#` specifier.
    let manifest: serde_json::Value = serde_json::from_slice(PRIVATE_PACKAGE_SCOPE_MANIFEST)
        .expect("the private package scope is valid JSON");
    let object = manifest.as_object().expect("a JSON object");
    for field in ["exports", "imports", "main", "module", "browser", "bin"] {
        assert!(
            !object.contains_key(field),
            "the private package scope must carry no {field}: it exists only to terminate \
             LOOKUP_PACKAGE_SCOPE"
        );
    }
    assert_eq!(
        object.get("type").and_then(serde_json::Value::as_str),
        Some("module")
    );
    assert_ne!(
        object.get("name").and_then(serde_json::Value::as_str),
        None,
        "a scope with no name would make LOOKUP_PACKAGE_SCOPE's result nameless rather than \
         unanswerable"
    );
}

// ---------------------------------------------------------------------------
// Export conditions, against the pinned interpreter
// ---------------------------------------------------------------------------

/// Reports what each of the three condition rows resolves to, for both import
/// kinds, from inside the private recipes directory.
const CONDITION_ROW_RESOLVER: &str = r#"import { createRequire } from "node:module";
const cjs = createRequire(import.meta.url);
for (const name of ["dual-sync", "dual-plain", "flagged"]) {
  let esm = "throw";
  try { esm = import.meta.resolve(name); } catch (error) { esm = `throw:${error.code}`; }
  let required = "throw";
  try { required = cjs.resolve(name); } catch (error) { required = `throw:${error.code}`; }
  console.log(`${name} ${esm} ${required}`);
}
"#;

/// The private layout, plus three packages whose `exports` isolate one
/// condition question each:
///
/// * `dual-sync` — `module-sync`, `import`, `require`, `default`. The reviewer's
///   first row: a conforming `module-sync` target beside a contradicting
///   `import` one.
/// * `dual-plain` — `import`, `require`, `default`. The second row: a
///   `createRequire` lands on the `require` target, not the `import` one.
/// * `flagged` — `development`, `default`. The third row: a requested
///   `development` condition is *not* applied unless the launch passes the
///   flag, so the default (a "production" build, in the idiom this pattern
///   comes from) is what a probe would have observed.
fn condition_workspace(scratch: &Scratch) -> (PathBuf, PathBuf) {
    let private = scratch.path().join("workspace");
    fs::create_dir(&private).expect("private directory");
    let layout = create_private_layout(&private).expect("the production private layout");
    let package = |name: &str, exports: &str, members: &[(&str, &str)]| {
        let directory = layout.modules.join(name);
        fs::create_dir_all(&directory).expect("package directory");
        fs::write(
            directory.join("package.json"),
            format!(
                "{{\"name\":\"{name}\",\"version\":\"1.0.0\",\"exports\":{{\".\":{exports}}}}}\n"
            ),
        )
        .expect("package manifest");
        for (file, body) in members {
            fs::write(directory.join(file), body).expect("package member");
        }
    };
    package(
        "dual-sync",
        "{\"module-sync\":\"./sync.mjs\",\"import\":\"./import.mjs\",\"require\":\
         \"./require.cjs\",\"default\":\"./default.mjs\"}",
        &[
            ("sync.mjs", "export const origin = \"module-sync\";\n"),
            ("import.mjs", "export const origin = \"import\";\n"),
            ("require.cjs", "module.exports = { origin: \"require\" };\n"),
            ("default.mjs", "export const origin = \"default\";\n"),
        ],
    );
    package(
        "dual-plain",
        "{\"import\":\"./import.mjs\",\"require\":\"./require.cjs\",\"default\":\
         \"./default.mjs\"}",
        &[
            ("import.mjs", "export const origin = \"import\";\n"),
            ("require.cjs", "module.exports = { origin: \"require\" };\n"),
            ("default.mjs", "export const origin = \"default\";\n"),
        ],
    );
    package(
        "flagged",
        "{\"development\":\"./development.mjs\",\"default\":\"./production.mjs\"}",
        &[
            (
                "development.mjs",
                "export const origin = \"development\";\n",
            ),
            ("production.mjs", "export const origin = \"production\";\n"),
        ],
    );
    fs::write(
        layout.recipes.join("resolve-rows.mjs"),
        CONDITION_ROW_RESOLVER,
    )
    .expect("row resolver");
    (private, layout.modules)
}

/// Runs the row resolver under the launch's own environment allowlist and the
/// given interpreter flags, and returns `name -> (esm, require)`.
fn resolve_condition_rows(
    node: &Path,
    private: &Path,
    flags: &[&str],
) -> BTreeMap<String, (String, String)> {
    let mut command = Command::new(node);
    command.args(flags);
    command
        .arg(private.join("recipes/resolve-rows.mjs"))
        .current_dir(private);
    command.env_clear();
    command.env("HOME", private);
    command.env("TMPDIR", private);
    command.env("LANG", "C");
    command.env("LC_ALL", "C");
    command.env("NODE_OPTIONS", "");
    let output = command.output().expect("run the row resolver");
    assert!(
        output.status.success(),
        "the row resolver exited {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split(' ');
            let name = fields.next()?;
            let esm = fields.next()?;
            let required = fields.next()?;
            Some((name.to_owned(), (esm.to_owned(), required.to_owned())))
        })
        .collect()
}

/// No dependency copies: the condition rows below predate declared dependency
/// specifiers and assert nothing about them.
fn no_dependency_roots() -> BTreeMap<String, PathBuf> {
    BTreeMap::new()
}

fn subject(specifier: &str, import_kind: ProbeImportKind) -> ResolutionSubject<'_> {
    ResolutionSubject {
        specifier,
        import_kind,
        dependencies: &[],
    }
}

fn reported(specifier: &str, import_kind: &str, esm: &str, require: &str) -> ReportedResolution {
    ReportedResolution {
        specifier: specifier.to_owned(),
        import_kind: import_kind.to_owned(),
        esm: esm.to_owned(),
        require: require.to_owned(),
        dependencies: Vec::new(),
    }
}

#[test]
fn the_recorded_conditions_come_from_the_pinned_interpreter() {
    // `conditions: ["import", "node"]` was a constant, and it was wrong: this
    // Node applies `module-sync` and `node-addons` as well, applies `require`
    // rather than `import` to a `createRequire`, and applies a *requested*
    // condition only when the launch passes the flag. Every one of those is
    // measured here from the bytes the harness launches.
    let Some(node) = resolution_node() else {
        eprintln!("export-condition observation test skipped: no node runtime");
        return;
    };
    let digest = digest_of(&fs::read(&node).expect("read the node bytes"));

    let bare = observe_conditions(&node, &digest, &[]).expect("the interpreter answers");
    let holds = |set: &[String], value: &str| set.iter().any(|entry| entry == value);
    for condition in ["node", "import"] {
        assert!(
            holds(&bare.esm, condition),
            "every Node applies {condition} to an import: {:?}",
            bare.esm
        );
    }
    for condition in ["node", "require"] {
        assert!(
            holds(&bare.require, condition),
            "every Node applies {condition} to a require: {:?}",
            bare.require
        );
    }
    assert!(!holds(&bare.esm, "require"), "{:?}", bare.esm);
    assert!(!holds(&bare.require, "import"), "{:?}", bare.require);
    assert_ne!(
        bare.esm, bare.require,
        "the two kinds cannot be described by one constant, which is the whole reason a recipe \
         declares its import kind"
    );
    assert!(
        !holds(&bare.esm, "development") && !holds(&bare.require, "development"),
        "an unrequested condition must not be applied"
    );

    // The flag half: a requested condition becomes applied because the launch
    // passes `--conditions=`, and that is measured too rather than assumed.
    let requested = observe_conditions(&node, &digest, &["development".to_owned()])
        .expect("the interpreter answers");
    assert!(
        holds(&requested.esm, "development") && holds(&requested.require, "development"),
        "a requested condition must be applied once the flag is passed: {requested:?}",
        requested = (&requested.esm, &requested.require)
    );

    // And the environment identity records exactly what was measured, tagged
    // by what each set means, rather than a single flat guess.
    let environment = probe_environment(
        &digest_of(NODE_STAND_IN),
        "v0.0.0-test",
        &["development".to_owned()],
        &requested,
    );
    let mut expected = vec!["requested:development".to_owned()];
    expected.extend(requested.esm.iter().map(|value| format!("esm:{value}")));
    expected.extend(
        requested
            .require
            .iter()
            .map(|value| format!("require:{value}")),
    );
    assert_eq!(
        environment.conditions, expected,
        "the recorded conditions must be the measured sets and nothing else"
    );
}

#[test]
fn the_pinned_interpreter_can_select_a_target_the_artifact_case_did_not() {
    // The reviewer's three-row table, measured on the pinned interpreter, and
    // then the refusal it forces.
    //
    // The false pass this closes: the artifact case is selected under the
    // requested conditions plus `default`, so a package whose `exports` lists
    // `module-sync` first has its `import` target certified by the Type Facts
    // witness while the interpreter hands the recipe the `module-sync` one. The
    // probe then observes a conforming file, contradicts nothing, and the
    // closure certifies on a target nobody verified.
    let Some(node) = resolution_node() else {
        eprintln!("export-condition row test skipped: no node runtime");
        return;
    };
    let scratch = Scratch::new("condition-rows");
    let (private, modules) = condition_workspace(&scratch);

    let bare = resolve_condition_rows(&node, &private, &[]);
    let flagged = resolve_condition_rows(
        &node,
        &private,
        &["--conditions=import", "--conditions=development"],
    );
    let row = |rows: &BTreeMap<String, (String, String)>, name: &str| {
        rows.get(name)
            .unwrap_or_else(|| panic!("the resolver reported no {name} row: {rows:?}"))
            .clone()
    };

    // Row 2, true of every Node: a `createRequire` lands on the `require`
    // target while an `import` lands on the `import` one.
    let (plain_esm, plain_require) = row(&bare, "dual-plain");
    assert!(
        plain_esm.ends_with("/import.mjs"),
        "an ESM import must select the import target: {plain_esm}"
    );
    assert!(
        plain_require.ends_with("/require.cjs"),
        "a createRequire must select the require target: {plain_require}"
    );

    // Row 3: a requested condition the launch does not pass is not applied, so
    // the default target answers — and passing the flag is what changes that.
    let (unflagged_esm, _) = row(&bare, "flagged");
    assert!(
        unflagged_esm.ends_with("/production.mjs"),
        "an unpassed `development` condition must not be applied: {unflagged_esm}"
    );
    let (flagged_esm, flagged_require) = row(&flagged, "flagged");
    assert!(
        flagged_esm.ends_with("/development.mjs") && flagged_require.ends_with("/development.mjs"),
        "passing --conditions=development must apply it: {flagged_esm} / {flagged_require}"
    );

    // Row 1, whenever this interpreter applies `module-sync` — which is
    // measured, not assumed. The flags do not rescue it: `module-sync` wins
    // over a requested `import` for both kinds, which is exactly why the flag
    // half is not the fix on its own.
    let digest = digest_of(&fs::read(&node).expect("read the node bytes"));
    let observed = observe_conditions(&node, &digest, &["import".to_owned()])
        .expect("the interpreter answers");
    let (sync_esm, sync_require) = row(&flagged, "dual-sync");
    if observed.esm.iter().any(|value| value == "module-sync") {
        assert!(
            sync_esm.ends_with("/sync.mjs") && sync_require.ends_with("/sync.mjs"),
            "this interpreter applies module-sync, so it must answer both kinds with that \
             target: {sync_esm} / {sync_require}"
        );
        // The gate refuses: the artifact case named the `import` target.
        let error = verify_reported_resolution(
            Some(&reported("dual-sync", "esm", &sync_esm, &sync_require)),
            subject("dual-sync", ProbeImportKind::Esm),
            &modules.join("dual-sync/import.mjs"),
            &no_dependency_roots(),
        )
        .expect_err("a probe that ran against another target must refuse the gate");
        assert!(
            matches!(&error, ProbeHarnessError::ConditionMismatch(message)
                if message.contains("different file than the Type Facts witness read")),
            "unexpected error: {error}"
        );
    }

    // The unconditional refusal, true on any Node: the `require` kind lands on
    // the `require` target, so a case that names the `import` one refuses.
    let error = verify_reported_resolution(
        Some(&reported(
            "dual-plain",
            "require",
            &plain_esm,
            &plain_require,
        )),
        subject("dual-plain", ProbeImportKind::Require),
        &modules.join("dual-plain/import.mjs"),
        &no_dependency_roots(),
    )
    .expect_err("a require that landed elsewhere must refuse the gate");
    assert!(
        matches!(&error, ProbeHarnessError::ConditionMismatch(_)),
        "unexpected error: {error}"
    );

    // And the matching cases pass, so the check is not simply always refusing.
    verify_reported_resolution(
        Some(&reported("dual-plain", "esm", &plain_esm, &plain_require)),
        subject("dual-plain", ProbeImportKind::Esm),
        &modules.join("dual-plain/import.mjs"),
        &no_dependency_roots(),
    )
    .expect("the target the interpreter actually selected must be accepted");
    verify_reported_resolution(
        Some(&reported(
            "dual-plain",
            "require",
            &plain_esm,
            &plain_require,
        )),
        subject("dual-plain", ProbeImportKind::Require),
        &modules.join("dual-plain/require.cjs"),
        &no_dependency_roots(),
    )
    .expect("the CommonJS target the interpreter actually selected must be accepted");
    verify_reported_resolution(
        Some(&reported("flagged", "esm", &flagged_esm, &flagged_require)),
        subject("flagged", ProbeImportKind::Esm),
        &modules.join("flagged/development.mjs"),
        &no_dependency_roots(),
    )
    .expect("a passed condition's target must be accepted");
}

#[test]
fn a_run_frame_that_reports_no_resolution_refuses_the_gate() {
    // A certification launch always asks, so silence is a worker that did not
    // answer rather than a launch that had nothing to prove. An echo of the
    // wrong specifier or the wrong kind is refused for the same reason: the
    // answer has to be to *this* question.
    let expected = Path::new("/nonexistent/node_modules/pkg/index.js");
    for (report, fragment) in [
        (None, "reported no module resolution"),
        (
            Some(reported(
                "other",
                "esm",
                "file:///x/index.js",
                "/x/index.js",
            )),
            "this launch asked for",
        ),
        (
            Some(reported(
                "pkg",
                "require",
                "file:///x/index.js",
                "/x/index.js",
            )),
            "this launch asked for",
        ),
        (
            Some(reported(
                "pkg",
                "esm",
                "unresolved:ERR_MODULE_NOT_FOUND",
                "",
            )),
            "not a local file",
        ),
        (
            Some(reported("pkg", "esm", "https://example.test/x.mjs", "")),
            "not a local file",
        ),
        (
            Some(reported("pkg", "esm", "file://host/x.mjs", "")),
            "not a local file",
        ),
    ] {
        let error = verify_reported_resolution(
            report.as_ref(),
            subject("pkg", ProbeImportKind::Esm),
            expected,
            &no_dependency_roots(),
        )
        .expect_err("an unusable resolution report must refuse");
        assert!(
            matches!(&error, ProbeHarnessError::ConditionMismatch(message)
                if message.contains(fragment)),
            "unexpected error for {fragment:?}: {error}"
        );
    }
}

#[test]
fn a_percent_escaped_file_url_names_the_same_file() {
    // Node percent-encodes what a URL must escape, so the comparison decodes
    // rather than string-matching. A malformed escape is not a local path, and
    // saying so is the fail-closed direction.
    assert_eq!(
        decode_file_url("file:///tmp/a%20b/index%2Emjs"),
        Some(PathBuf::from("/tmp/a b/index.mjs"))
    );
    assert_eq!(
        decode_file_url("file:///tmp/x.mjs?v=1#top"),
        Some(PathBuf::from("/tmp/x.mjs"))
    );
    for malformed in [
        "file:///tmp/%zz",
        "file:///tmp/%4",
        "file://host/tmp/x",
        "/tmp/x",
        "data:text/javascript,0",
    ] {
        assert_eq!(
            decode_file_url(malformed),
            None,
            "{malformed} is not a path"
        );
    }
}

#[test]
fn an_export_condition_that_is_not_a_plain_name_refuses_before_it_reaches_the_interpreter() {
    // Requested conditions arrive from the certification request and are
    // interpolated into a `--conditions=` flag and into the observation
    // packages' manifests, so a name outside the conservative charset refuses
    // rather than being passed through.
    for condition in [
        "",
        "with space",
        "with\"quote",
        "with\nnewline",
        "a/b",
        &"x".repeat(65),
    ] {
        let error =
            plain_condition_name(condition).expect_err("an unusable condition name must refuse");
        assert!(
            matches!(&error, ProbeHarnessError::Configuration(message)
                if message.contains("plain condition name")),
            "unexpected error for {condition:?}: {error}"
        );
    }
    for condition in ["import", "module-sync", "react-server", "node_18", "v1.2"] {
        plain_condition_name(condition).expect("a plain condition name is accepted");
    }
}

#[test]
fn the_production_layout_writes_a_package_scope_beside_the_harness_and_the_recipes() {
    // Both directories hold importing modules: the worker and the harness
    // module in one, every recipe in the other. `node_modules/` deliberately
    // gets none — `LOOKUP_PACKAGE_SCOPE` already returns null at a
    // `node_modules` segment, and the snapshot copy carries the analyzed
    // package's own authenticated manifest.
    let scratch = Scratch::new("layout");
    let private = scratch.path().join("workspace");
    fs::create_dir(&private).expect("private directory");
    let layout = create_private_layout(&private).expect("the production private layout");
    for directory in [&layout.harness, &layout.recipes] {
        assert_eq!(
            fs::read(directory.join("package.json")).expect("a private package scope"),
            PRIVATE_PACKAGE_SCOPE_MANIFEST,
            "{} must hold the private package scope",
            directory.display()
        );
    }
    assert!(
        !layout.modules.join("package.json").exists(),
        "the private node_modules must not be given a package scope"
    );
}
