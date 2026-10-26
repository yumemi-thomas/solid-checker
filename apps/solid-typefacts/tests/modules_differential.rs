//! The S3 differential gate: the `modules` and `sources` operations of the Rust
//! producer (tsrs) must equal the Go producer's on the repository's fixture
//! projects, modulo the documented normalizations.
//!
//! * Default: a deterministic subset (every `TSRS_DIFF_STRIDE`-th project, 12 by
//!   default, plus every project that uses `references` or `paths`).
//! * `TSRS_DIFF_ALL=1`: every `fixtures/**/tsconfig.json` (about 393).
//! * The reference producer is `SOLID_TYPEFACTS_REFERENCE_BIN` (default: the archived Go producer, see common::reference_binary);
//!   without one the test prints a notice and passes vacuously, as the other
//!   differential tests do.
//!
//! Normalizations (each is a category in the printed report, not a failure):
//!
//! * `bundled:///libs/...` and `bundled:/libs/...` spell the same default-library
//!   path; both producers clean paths, so they already agree, and the report
//!   proves it by counting rewrites (expected 0).
//! * The default library set differs between the pinned TypeScript snapshots
//!   (plan section 1.6): files only one producer includes are `lib-set`
//!   differences. Library files both include must still agree field for field.
//!
//! Everything else is a real difference and fails the test with a listing.

mod common;

use common::{frame, frames, request};
use solid_typefacts::backend::ClosureBackend;
use solid_typefacts::context::Context;
use solid_typefacts::project::Project;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use typefacts::v3::{ModuleGraphRequest, Operation, Response};
use typefacts::{ModuleFact, ModuleImportFact};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn go_binary() -> Option<PathBuf> {
    common::reference_binary("modules differential")
}

fn rust_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_solid-typefacts"))
}

fn collect(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        if name == "node_modules" || name == ".git" {
            continue;
        }
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            collect(&path, found);
        } else if name == "tsconfig.json" {
            found.push(path);
        }
    }
}

fn projects() -> Vec<PathBuf> {
    let mut all = Vec::new();
    collect(&repo_root().join("fixtures"), &mut all);
    all.sort();
    if std::env::var_os("TSRS_DIFF_ALL").is_some_and(|value| value == "1") {
        return all;
    }
    let stride: usize = std::env::var("TSRS_DIFF_STRIDE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(12)
        .max(1);
    all.into_iter()
        .enumerate()
        .filter(|(index, path)| {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            index % stride == 0 || text.contains("\"references\"") || text.contains("\"paths\"")
        })
        .map(|(_, path)| path)
        .collect()
}

struct Outcome {
    stdout: Vec<u8>,
    stderr: String,
    code: Option<i32>,
}

fn run(binary: &Path, args: &[&str], stdin: &[u8]) -> Outcome {
    let mut child = Command::new(binary)
        .args(args)
        .env_remove("SOLID_TYPEFACTS_TIMINGS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let bytes = stdin.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = input.write_all(&bytes);
    });
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap();
    Outcome {
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code(),
    }
}

/// open, modules (every import, with packages), sources, close.
fn script(project: &str) -> Vec<u8> {
    let with = |id: u64, operation: Operation, edit: &dyn Fn(&mut typefacts::v3::Request)| {
        let mut value = request(id, operation, 1);
        value.project_id = project.to_owned();
        edit(&mut value);
        frame(&value)
    };
    [
        with(1, Operation::Open, &|_| {}),
        with(2, Operation::Modules, &|r| {
            r.module_graph = Some(ModuleGraphRequest {
                imports: true,
                import_paths: Vec::new(),
                packages: true,
                distinct_bare_imports: false,
                omit_modules: false,
            });
        }),
        with(3, Operation::Modules, &|r| {
            r.module_graph = Some(ModuleGraphRequest::default());
        }),
        with(4, Operation::Sources, &|_| {}),
        with(5, Operation::Close, &|_| {}),
    ]
    .concat()
}

struct Run {
    handshake: Vec<u8>,
    responses: Vec<Response>,
    stderr: String,
    code: Option<i32>,
}

fn drive(binary: &Path, project: &str) -> Run {
    drive_with(binary, project, &[])
}

fn drive_with(binary: &Path, project: &str, extra: &[&str]) -> Run {
    let mut args = vec!["-project", project, "-inline-sources"];
    args.extend_from_slice(extra);
    let outcome = run(binary, &args, &script(project));
    let mut parts = frames(&outcome.stdout).into_iter();
    let handshake = parts.next().unwrap_or_default();
    Run {
        handshake,
        responses: parts
            .map(|payload| typefacts::decode(&payload).expect("a response frame"))
            .collect(),
        stderr: outcome.stderr,
        code: outcome.code,
    }
}

#[derive(Default)]
struct Report {
    counts: BTreeMap<String, usize>,
    examples: BTreeMap<String, Vec<String>>,
    lib_only_go: BTreeMap<String, usize>,
    lib_only_rust: BTreeMap<String, usize>,
    failures: BTreeSet<String>,
}

impl Report {
    fn note(&mut self, category: &str, project: &str, detail: String) {
        *self.counts.entry(category.to_owned()).or_default() += 1;
        let examples = self.examples.entry(category.to_owned()).or_default();
        if examples.len() < 3 {
            examples.push(format!("{project}: {detail}"));
        }
    }

    /// A difference that is not covered by a documented normalization.
    fn fail(&mut self, category: &str, project: &str, detail: String) {
        self.failures.insert(category.to_owned());
        self.note(category, project, detail);
    }
}

fn is_lib(path: &str) -> bool {
    path.starts_with("bundled:")
}

fn normal(path: &str) -> String {
    path.replace("bundled:///", "bundled:/")
}

fn module_fields(go: &ModuleFact, rust: &ModuleFact) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if go.declaration_file != rust.declaration_file {
        fields.push("declarationFile");
    }
    if go.format != rust.format {
        fields.push("format");
    }
    if go.project_reference != rust.project_reference {
        fields.push("projectReference");
    }
    if go.redirect_targets != rust.redirect_targets {
        fields.push("redirectTargets");
    }
    fields
}

fn import_fields(go: &ModuleImportFact, rust: &ModuleImportFact) -> Vec<&'static str> {
    let mut fields = Vec::new();
    macro_rules! field {
        ($name:literal, $member:ident) => {
            if go.$member != rust.$member {
                fields.push($name);
            }
        };
    }
    field!("text", text);
    field!("resolution", resolution);
    field!("resolvedPath", resolved_path);
    field!("includedPath", included_path);
    field!("symlinkPath", symlink_path);
    field!("extension", extension);
    field!("tsExtension", ts_extension);
    field!("pathsPattern", paths_pattern);
    field!("package", package);
    field!("resolverPackage", resolver_package);
    fields
}

fn compare(
    project: &str,
    absolute: &str,
    config_source: Option<&str>,
    go: &Run,
    rust: &Run,
    report: &mut Report,
) {
    if go.handshake != rust.handshake {
        report.fail("handshake", project, "bytes differ".into());
    }
    // A project one producer cannot open must fail identically in the other.
    if go.code != rust.code || go.stderr != rust.stderr {
        report.fail(
            "open-failure",
            project,
            format!(
                "go {:?} {:?}; rust {:?} {:?}",
                go.code, go.stderr, rust.code, rust.stderr
            ),
        );
        return;
    }
    if go.code != Some(0) {
        report.note("open-failure-identical", project, go.stderr.clone());
        return;
    }
    if go.responses.len() != rust.responses.len() {
        report.fail(
            "response-count",
            project,
            format!("{} vs {}", go.responses.len(), rust.responses.len()),
        );
        return;
    }
    for (index, (go_response, rust_response)) in
        go.responses.iter().zip(&rust.responses).enumerate()
    {
        if go_response.ok != rust_response.ok || go_response.error != rust_response.error {
            report.fail(
                "response-status",
                project,
                format!(
                    "request #{index}: go {:?}/{:?}, rust {:?}/{:?}",
                    go_response.ok, go_response.error, rust_response.ok, rust_response.error
                ),
            );
            return;
        }
    }

    // sources: byte equality, path for path.
    let (go_sources, rust_sources) = (&go.responses[3].sources, &rust.responses[3].sources);
    if go_sources == rust_sources {
        report.note(
            "sources-identical",
            project,
            format!("{} files", go_sources.len()),
        );
    } else {
        report.fail(
            "sources",
            project,
            format!(
                "go {:?}; rust {:?}",
                go_sources.iter().map(|s| &s.path).collect::<Vec<_>>(),
                rust_sources.iter().map(|s| &s.path).collect::<Vec<_>>()
            ),
        );
    }

    for (label, index) in [("inventory", 2_usize), ("imports", 1_usize)] {
        let (go_modules, rust_modules) =
            (&go.responses[index].modules, &rust.responses[index].modules);
        let mut rewrites = 0;
        let rust_by_path: BTreeMap<String, &ModuleFact> = rust_modules
            .iter()
            .map(|module| {
                if module.path.contains("bundled:///") {
                    rewrites += 1;
                }
                (normal(&module.path), module)
            })
            .collect();
        if rewrites != 0 {
            report.note(
                "bundled-prefix-rewritten",
                project,
                format!("{rewrites} paths"),
            );
        }
        let go_by_path: BTreeMap<String, &ModuleFact> = go_modules
            .iter()
            .map(|module| (normal(&module.path), module))
            .collect();
        let libs = go_by_path.keys().filter(|path| is_lib(path)).count();
        report.note(
            &format!("{label}: modules compared (go total / go library files / rust total)"),
            project,
            format!("{} / {libs} / {}", go_by_path.len(), rust_by_path.len()),
        );
        let mut lib_set_differs = false;
        for path in go_by_path
            .keys()
            .filter(|path| !rust_by_path.contains_key(*path))
        {
            if is_lib(path) {
                lib_set_differs = true;
                *report.lib_only_go.entry(path.clone()).or_default() += 1;
            } else {
                report.fail(
                    &format!("{label}: project module only in go"),
                    project,
                    path.clone(),
                );
            }
        }
        for path in rust_by_path
            .keys()
            .filter(|path| !go_by_path.contains_key(*path))
        {
            if is_lib(path) {
                lib_set_differs = true;
                *report.lib_only_rust.entry(path.clone()).or_default() += 1;
            } else {
                report.fail(
                    &format!("{label}: project module only in rust"),
                    project,
                    path.clone(),
                );
            }
        }
        if lib_set_differs {
            report.note("lib-set", project, label.to_owned());
        }
        for (path, go_module) in &go_by_path {
            let Some(rust_module) = rust_by_path.get(path) else {
                continue;
            };
            let fields = module_fields(go_module, rust_module);
            if !fields.is_empty() {
                report.fail(
                    &format!("{label}: module fields {}", fields.join(",")),
                    project,
                    format!("{path}: go {go_module:?} rust {rust_module:?}"),
                );
            }
        }
    }

    // imports: row for row on the (path, start) key.
    let (go_imports, rust_imports) = (
        &go.responses[1].module_imports,
        &rust.responses[1].module_imports,
    );
    let key = |fact: &ModuleImportFact| {
        (
            fact.specifier.path.to_string(),
            fact.specifier.start_byte,
            fact.specifier.end_byte,
        )
    };
    let rust_rows: BTreeMap<_, _> = rust_imports.iter().map(|fact| (key(fact), fact)).collect();
    let go_rows: BTreeMap<_, _> = go_imports.iter().map(|fact| (key(fact), fact)).collect();
    let mut compared = 0;
    for (row_key, go_fact) in &go_rows {
        match rust_rows.get(row_key) {
            None => report.fail(
                "imports: row only in go",
                project,
                format!("{row_key:?} {go_fact:?}"),
            ),
            Some(rust_fact) => {
                compared += 1;
                let fields = import_fields(go_fact, rust_fact);
                if !fields.is_empty() {
                    report.fail(
                        &format!("imports: fields {}", fields.join(",")),
                        project,
                        format!("go {go_fact:?} rust {rust_fact:?}"),
                    );
                }
            }
        }
    }
    for (row_key, rust_fact) in &rust_rows {
        if !go_rows.contains_key(row_key) {
            report.fail(
                "imports: row only in rust",
                project,
                format!("{row_key:?} {rust_fact:?}"),
            );
        }
    }
    if go_imports.len() != rust_imports.len() && go_rows.len() == go_imports.len() {
        report.fail(
            "imports: row count",
            project,
            format!("{} vs {}", go_imports.len(), rust_imports.len()),
        );
    }
    if go.responses[1].unknown_import_paths != rust.responses[1].unknown_import_paths {
        report.fail("unknownImportPaths", project, "differ".into());
    }
    report.note("imports-rows-compared", project, format!("{compared}"));

    // Source digests: hash of Go's own source bytes against the digests the
    // in-process Rust backend computes from its program.
    let mut backend = match Project::open(&Context::new(), absolute, config_source, None) {
        Ok(backend) => backend,
        Err(error) => {
            report.fail("digest-open", project, error);
            return;
        }
    };
    let digests = backend.source_digests(&Context::new()).unwrap();
    let expected: Vec<_> = go_sources
        .iter()
        .map(|source| typefacts::producer::source_digest(&source.path, &source.source))
        .collect();
    if digests == expected {
        report.note(
            "digests-identical",
            project,
            format!("{} files", digests.len()),
        );
    } else {
        report.fail(
            "digests",
            project,
            "differ from sha256 of Go's sources".into(),
        );
    }
}

#[test]
fn modules_and_sources_equal_the_go_producer() {
    let Some(go) = go_binary() else { return };
    let rust = rust_binary();
    let projects = projects();
    eprintln!("modules differential over {} projects", projects.len());
    let report = Mutex::new(Report::default());
    let queue = Mutex::new(projects.iter().collect::<Vec<_>>().into_iter());
    let workers = std::thread::available_parallelism()
        .map_or(2, std::num::NonZero::get)
        .clamp(1, 6);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let Some(config) = queue.lock().unwrap().next() else {
                        return;
                    };
                    let project = config.to_string_lossy().into_owned();
                    let (go_run, rust_run) = (drive(&go, &project), drive(&rust, &project));
                    let relative = config
                        .strip_prefix(repo_root().join("fixtures"))
                        .unwrap_or(config)
                        .to_string_lossy()
                        .into_owned();
                    compare(
                        &relative,
                        &project,
                        None,
                        &go_run,
                        &rust_run,
                        &mut report.lock().unwrap(),
                    );
                }
            });
        }
    });
    let report = report.into_inner().unwrap();
    eprintln!(
        "\n--- modules/sources differential report ({} projects) ---",
        projects.len()
    );
    for (category, count) in &report.counts {
        eprintln!("{count:5}  {category}");
        for example in report.examples.get(category).into_iter().flatten() {
            eprintln!("         e.g. {example}");
        }
    }
    let summarize = |label: &str, files: &BTreeMap<String, usize>| {
        eprintln!("{label}: {} distinct files", files.len());
        for (file, count) in files {
            eprintln!("         {file} ({count} projects)");
        }
    };
    summarize("library files only in the Go producer", &report.lib_only_go);
    summarize(
        "library files only in the Rust producer",
        &report.lib_only_rust,
    );
    assert!(
        report.failures.is_empty(),
        "non-normalized differences: {:?}",
        report.failures
    );
}

/// `-project-config`: the bytes of a configuration that does not exist on disk
/// stand in for it at its logical path (ADR 0303). Both producers must open the
/// same program from them.
#[test]
fn project_config_overlay_matches_go() {
    let Some(go) = go_binary() else { return };
    let rust = rust_binary();
    let fixtures = repo_root().join("fixtures");
    let mut compared = 0;
    for name in [
        "reactive-ir/closed-export-alias-load",
        "reactive-ir/arrow-bound-helper-workspace-link",
        "reactive-ir/inferred-host-alias",
    ] {
        let directory = fixtures.join(name);
        let Ok(bytes) = std::fs::read(directory.join("tsconfig.json")) else {
            continue;
        };
        let holder = std::env::temp_dir().join(format!(
            "solid-typefacts-rs-config-{}-{}.json",
            std::process::id(),
            name.replace('/', "-")
        ));
        std::fs::write(&holder, &bytes).unwrap();
        let config_text = String::from_utf8_lossy(&bytes).into_owned();
        let logical = directory.join("tsconfig.logical.json");
        assert!(
            !logical.exists(),
            "the logical config must not exist on disk"
        );
        let (logical, holder_arg) = (
            logical.to_string_lossy().into_owned(),
            holder.to_string_lossy().into_owned(),
        );
        let extra = ["-project-config", holder_arg.as_str()];
        let (go_run, rust_run) = (
            drive_with(&go, &logical, &extra),
            drive_with(&rust, &logical, &extra),
        );
        let mut report = Report::default();
        compare(
            name,
            &logical,
            Some(&config_text),
            &go_run,
            &rust_run,
            &mut report,
        );
        let _ = std::fs::remove_file(&holder);
        assert!(
            report.failures.is_empty(),
            "{name}: {:?} {:?}",
            report.failures,
            report.examples
        );
        assert_eq!(go_run.code, Some(0), "{name}: {}", go_run.stderr);
        assert!(!go_run.responses[2].modules.is_empty());
        compared += 1;
    }
    assert!(compared > 0, "no fixture project to overlay");
}

/// `update`: the same edits sent to both producers must yield the same
/// affected sets (the declaration-shape cutoff, the import-graph walk and the
/// full-rebuild paths), and the same program afterwards.
#[test]
fn update_affected_sets_match_go() {
    let Some(go) = go_binary() else { return };
    let rust = rust_binary();
    let selected: Vec<PathBuf> = projects();
    let report = Mutex::new(Report::default());
    let queue = Mutex::new(selected.iter().collect::<Vec<_>>().into_iter());
    let workers = std::thread::available_parallelism()
        .map_or(2, std::num::NonZero::get)
        .clamp(1, 6);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let Some(config) = queue.lock().unwrap().next() else {
                        return;
                    };
                    let project = config.to_string_lossy().into_owned();
                    let label = config
                        .strip_prefix(repo_root().join("fixtures"))
                        .unwrap_or(config)
                        .to_string_lossy()
                        .into_owned();
                    update_case(&go, &rust, &label, &project, &mut report.lock().unwrap());
                }
            });
        }
    });
    let report = report.into_inner().unwrap();
    eprintln!(
        "\n--- update differential report ({} projects) ---",
        selected.len()
    );
    for (category, count) in &report.counts {
        eprintln!("{count:5}  {category}");
        for example in report.examples.get(category).into_iter().flatten() {
            eprintln!("         e.g. {example}");
        }
    }
    assert!(
        report.failures.is_empty(),
        "non-normalized differences: {:?}",
        report.failures
    );
}

fn update_case(go: &Path, rust: &Path, label: &str, project: &str, report: &mut Report) {
    let baseline = drive(go, project);
    if baseline.code != Some(0) || baseline.responses.len() < 4 {
        return;
    }
    let sources = &baseline.responses[3].sources;
    // Edit up to three files, spread over the path order.
    let mut picks: Vec<&typefacts::v3::SourceFile> = Vec::new();
    for index in [0, sources.len() / 2, sources.len().saturating_sub(1)] {
        if let Some(source) = sources.get(index)
            && !picks.iter().any(|picked| picked.path == source.path)
        {
            picks.push(source);
        }
    }
    let text =
        |source: &typefacts::v3::SourceFile| String::from_utf8_lossy(&source.source).into_owned();
    let mut steps: Vec<Vec<typefacts::v3::FileChange>> = Vec::new();
    let mut version = 0;
    let mut change = |source: &typefacts::v3::SourceFile, suffix: &str| {
        version += 1;
        typefacts::v3::FileChange {
            path: source.path.clone(),
            source: format!("{}{suffix}", text(source)).into_bytes(),
            deleted: false,
            version,
        }
    };
    for source in &picks {
        // A trivia edit, then one that adds an export: the shape moves.
        steps.push(vec![change(source, "\n// probe\n")]);
        steps.push(vec![change(source, "\nexport const __probe = 1;\n")]);
    }
    if picks.len() >= 2 {
        // Two files at once take the full rebuild path.
        steps.push(vec![
            change(picks[0], "\n// both\n"),
            change(picks[1], "\n// both\n"),
        ]);
    }
    let mut bytes = Vec::new();
    let with = |id: u64,
                operation: Operation,
                generation: u64,
                edit: &dyn Fn(&mut typefacts::v3::Request)| {
        let mut value = request(id, operation, generation);
        value.project_id = project.to_owned();
        edit(&mut value);
        frame(&value)
    };
    bytes.extend(with(1, Operation::Open, 1, &|_| {}));
    for (index, step) in steps.iter().enumerate() {
        bytes.extend(with(
            10 + index as u64,
            Operation::Update,
            2 + index as u64,
            &|r| {
                r.changes = step.clone();
            },
        ));
    }
    let end = 2 + steps.len() as u64;
    bytes.extend(with(100, Operation::Sources, end, &|_| {}));
    bytes.extend(with(101, Operation::Modules, end, &|r| {
        r.module_graph = Some(ModuleGraphRequest {
            imports: true,
            import_paths: Vec::new(),
            packages: true,
            distinct_bare_imports: false,
            omit_modules: false,
        });
    }));
    bytes.extend(with(102, Operation::Close, end, &|_| {}));
    let args = ["-project", project, "-inline-sources"];
    let (go_out, rust_out) = (run(go, &args, &bytes), run(rust, &args, &bytes));
    let decode = |outcome: &Outcome| -> Vec<Response> {
        frames(&outcome.stdout)
            .into_iter()
            .skip(1)
            .map(|payload| typefacts::decode(&payload).expect("a response frame"))
            .collect()
    };
    let (go_responses, rust_responses) = (decode(&go_out), decode(&rust_out));
    if go_responses.len() != rust_responses.len() {
        report.fail(
            "update: response count",
            label,
            format!("{} vs {}", go_responses.len(), rust_responses.len()),
        );
        return;
    }
    for (index, (left, right)) in go_responses.iter().zip(&rust_responses).enumerate() {
        let kind = if (1..=steps.len()).contains(&index) {
            "update"
        } else {
            "tail"
        };
        if left.ok != right.ok || left.error != right.error {
            report.fail(
                &format!("{kind}: status"),
                label,
                format!(
                    "step {index}: go {:?}/{:?} rust {:?}/{:?}",
                    left.ok, left.error, right.ok, right.error
                ),
            );
            continue;
        }
        if left.affected != right.affected {
            report.fail(
                &format!("{kind}: affected set"),
                label,
                format!(
                    "step {index}: go {:?} rust {:?}",
                    left.affected, right.affected
                ),
            );
        } else if kind == "update" {
            report.note(
                "update: affected sets identical",
                label,
                format!("step {index}: {} files", left.affected.len()),
            );
        }
        if left.sources != right.sources {
            report.fail(
                "tail: sources after updates",
                label,
                format!("step {index}"),
            );
        }
        if left.modules != right.modules || left.module_imports != right.module_imports {
            report.fail(
                "tail: module graph after updates",
                label,
                format!("step {index}"),
            );
        }
    }
}
