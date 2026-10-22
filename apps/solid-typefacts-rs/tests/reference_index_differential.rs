//! Go-vs-Rust differential for the two reference-index defects both producers
//! had (Go commit 349fc4c9c, ported in the Rust producer by S4c):
//!
//! 1. a broad invalidation (a non-cutoff edit to a file with more than 64
//!    importers) dropped the merged index but kept the contributions of
//!    unaffected files, which then came back with no references;
//! 2. a symbol pruned by `ReleaseAnalysisState` answered an empty list.
//!
//! The scenario: `hub.ts` has 70 importers; `kept.ts` is unrelated and keeps
//! its retained contribution. Each producer is opened, analyzed with
//! `references` demands on `kept.ts`, `hub.ts` and one importer, updated with a
//! non-cutoff edit of `hub.ts` (every importer is affected, `kept.ts` is not),
//! analyzed again over the same demands, and the symbol/reference rows are
//! compared. `analyze` releases analysis state after each answer, so the
//! second analysis also asks about symbols the first release pruned.
//!
//! The Go producer is `bin/solid-typefacts` (or `SOLID_TYPEFACTS_GO_BIN`); it
//! must include 349fc4c9c or this test fails on the Go side's lost references.
//! Without a Go binary the Rust side is still checked against the known
//! answer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use typefacts::v3::{EntityDemand, FileChange};
use typefacts::{AnalysisDemand, FactTable, Location, Producer, Session};

const CONFIG: &str = r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}"#;
const KEPT: &str = "export const shared = 1;\nexport const a = shared;\nexport const b = shared + shared;\nexport const c = a;\nexport const d = c;\n";
const IMPORTER: &str = "import { hub } from \"./hub\";\nexport const v = hub;\n";
const HUB: &str = "export const hub = 1;\n";
const HUB_EDITED: &str = "export const hub = \"s\";\n";

fn go_binary() -> Option<PathBuf> {
    let path = std::env::var_os("SOLID_TYPEFACTS_GO_BIN").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin/solid-typefacts"),
        PathBuf::from,
    );
    path.is_file().then_some(path)
}

fn rust_binary() -> PathBuf {
    std::env::var_os("TSRS_DIFF_RUST_BIN").map_or_else(
        || PathBuf::from(env!("CARGO_BIN_EXE_solid-typefacts")),
        PathBuf::from,
    )
}

struct Project {
    root: PathBuf,
}

impl Project {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("solid-typefacts-rs-refdiff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(root.join("tsconfig.json"), CONFIG).unwrap();
        std::fs::write(root.join("hub.ts"), HUB).unwrap();
        std::fs::write(root.join("kept.ts"), KEPT).unwrap();
        for index in 0..70 {
            std::fs::write(root.join(format!("imp{index:02}.ts")), IMPORTER).unwrap();
        }
        Self { root }
    }

    fn path(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().into_owned()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn demand(project: &Project, name: &str, source: &str, needle: &str, last: bool) -> EntityDemand {
    let start = if last {
        source.rfind(needle)
    } else {
        source.find(needle)
    }
    .unwrap();
    EntityDemand {
        location: Location {
            path: project.path(name).as_str().into(),
            start_byte: start as u64,
            end_byte: (start + needle.len()) as u64,
        },
        symbol: true,
        references: true,
        ..EntityDemand::default()
    }
}

/// Later analyses also demand symbols of `kept.ts` the first did not, so
/// those are answered by the post-update reference index. (Symbols demanded
/// before, in a file the update left alone, keep their retained rows and never
/// ask the backend again.)
fn demands(project: &Project, hub_source: &str, phase: usize) -> Vec<EntityDemand> {
    // The single-letter identifier at the end of `needle` (`c = a;`).
    let use_of = |needle: &str| {
        let at = KEPT.find(needle).unwrap() + needle.len() - 2;
        let mut demand = demand(project, "kept.ts", KEPT, "shared", false);
        demand.location.start_byte = at as u64;
        demand.location.end_byte = at as u64 + 1;
        demand
    };
    let mut demands = vec![
        demand(project, "kept.ts", KEPT, "shared", false),
        demand(project, "kept.ts", KEPT, "shared", true),
        demand(project, "kept.ts", KEPT, "b", false),
        demand(project, "hub.ts", hub_source, "hub", false),
        demand(project, "imp00.ts", IMPORTER, "hub", true),
        demand(project, "imp69.ts", IMPORTER, "v", false),
    ];
    if phase >= 1 {
        demands.push(use_of("c = a;")); // symbol `a`, one reference
    }
    if phase >= 3 {
        demands.push(use_of("d = c;")); // symbol `c`, one reference
    }
    demands.sort_by(|left, right| {
        (&*left.location.path, left.location.start_byte)
            .cmp(&(&*right.location.path, right.location.start_byte))
    });
    demands
}

/// Symbol id -> (declaration (path, start), reference locations) for every
/// symbol the table holds; ids are durable, so no renaming is needed.
type Rows = BTreeMap<String, (Vec<(String, u64)>, Vec<(String, u64, u64)>)>;

fn rows(table: &FactTable) -> Rows {
    table
        .symbols()
        .map(|symbol| {
            (
                symbol.id().to_owned(),
                (
                    symbol
                        .declarations()
                        .iter()
                        .map(|declaration| {
                            (
                                declaration.location.path.to_string(),
                                declaration.location.start_byte,
                            )
                        })
                        .collect(),
                    symbol
                        .references()
                        .map(|location| {
                            (
                                location.path.to_string(),
                                location.start_byte,
                                location.end_byte,
                            )
                        })
                        .collect(),
                ),
            )
        })
        .collect()
}

/// The analyses of one producer: cold; after a broad (non-cutoff) edit of
/// `hub.ts`; after the edit is reverted (broad again); after a small edit of
/// one importer.
fn drive(binary: &Path, project: &Project) -> Vec<Rows> {
    let mut session = Session::open(
        Producer::at(binary),
        project.path("tsconfig.json"),
        Vec::<FileChange>::new(),
    )
    .expect("open");
    let mut analyses = Vec::new();
    let mut analyze = |session: &mut Session, hub_source: &str, phase: usize| {
        let table = session
            .analyze(&AnalysisDemand {
                entities: demands(project, hub_source, phase),
            })
            .expect("analyze");
        analyses.push(rows(&table));
    };
    let edit = |name: &str, source: &str, version: u64| FileChange {
        path: project.path(name),
        source: source.as_bytes().to_vec(),
        deleted: false,
        version,
    };
    analyze(&mut session, HUB, 0);
    session
        .update([edit("hub.ts", HUB_EDITED, 1)])
        .expect("broad update");
    analyze(&mut session, HUB_EDITED, 1);
    session
        .update([edit("hub.ts", HUB, 2)])
        .expect("revert update");
    analyze(&mut session, HUB, 2);
    session
        .update([edit(
            "imp05.ts",
            &format!("{IMPORTER}export const w = 1;\n"),
            1,
        )])
        .expect("small update");
    analyze(&mut session, HUB, 3);
    let _ = session.close();
    analyses
}

/// The references of `kept.ts`'s `name`, found by its declaration so the
/// answer is the producer's, not an id this test computed.
fn references_of<'a>(project: &Project, rows: &'a Rows, name: &str) -> &'a [(String, u64, u64)] {
    let declaration = (
        project.path("kept.ts"),
        KEPT.find(&format!("const {name} ")).unwrap() as u64 + "const ".len() as u64,
    );
    rows.values()
        .find(|(declarations, _)| declarations.contains(&declaration))
        .map_or(&[], |(_, references)| references.as_slice())
}

/// What a correct producer answers: `shared` is read three times (once in
/// `a`, twice in `b`; its declaration is not a reference), and `a` and `c`
/// once each once they are demanded.
fn assert_known_answers(who: &str, project: &Project, analyses: &[Rows]) {
    for (phase, rows) in analyses.iter().enumerate() {
        let want: &[(&str, usize, usize)] = &[("shared", 3, 0), ("a", 1, 1), ("c", 1, 3)];
        for (name, count, from_phase) in want {
            if phase < *from_phase {
                continue;
            }
            let references = references_of(project, rows, name);
            assert_eq!(
                references.len(),
                *count,
                "{who} phase {phase}: `{name}` references {references:?}"
            );
        }
    }
}

#[test]
fn broad_invalidation_and_pruned_symbols_answer_like_go() {
    let project = Project::new();
    let rust = drive(&rust_binary(), &project);
    assert_known_answers("rust", &project, &rust);
    let Some(go) = go_binary() else {
        eprintln!(
            "NOTICE: no Go producer; reference-index differential checked against the known answers only"
        );
        return;
    };
    let go_rows = drive(&go, &project);
    assert_known_answers("go", &project, &go_rows);
    assert_eq!(go_rows.len(), rust.len());
    for (phase, (go_analysis, rust_analysis)) in go_rows.iter().zip(&rust).enumerate() {
        assert_eq!(
            go_analysis, rust_analysis,
            "phase {phase}: Go and Rust symbol rows differ"
        );
    }
}
