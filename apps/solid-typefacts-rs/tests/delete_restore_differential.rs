//! Go-vs-Rust differential for a file that leaves the program and comes back
//! (Go commit 3991f6919). A restored file satisfies an import that was
//! unresolved when the importer's answers were computed; both producers must
//! take the full-rebuild path so the importer's retained answers are
//! recomputed, and must answer every fact family the same way.
//!
//! Each producer is opened over `dependency.ts`, `consumer.ts` and
//! `unrelated.ts`, analyzed cold, updated with a deletion of `dependency.ts`,
//! analyzed, updated with the restoration, and analyzed again. The demanded
//! families run one per session (symbol plus that family at every binding name
//! and call callee, so no other family's recorded dependencies can mask a stale
//! answer) and all together. The entity rows of each analysis are compared
//! between producers, and the restored analysis must equal the cold one on
//! each producer.
//!
//! The Go producer is `bin/solid-typefacts` (or `SOLID_TYPEFACTS_GO_BIN`); it
//! must include 3991f6919. Without a Go binary the Rust side is checked against
//! its own cold analysis only.

use std::path::{Path, PathBuf};
use typefacts::v3::{EntityDemand, FileChange};
use typefacts::{AnalysisDemand, EntityFact, Location, Producer, Session};

const CONFIG: &str = r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":true},"include":["*.ts"]}"#;
const DEPENDENCY: &str = "export function value(): number { return 1; }\nexport class Thing { n = 1 }\nexport const list = [1, 2];\nexport const pair: [number, string] = [1, \"a\"];\nexport const konst = 3;\n";
const CONSUMER: &str = "import { value, Thing, list, pair, konst } from \"./dependency\";\nexport const result = value();\nexport const made = new Thing();\nexport const l = list;\nexport const p = pair;\nexport const k = konst;\nexport const f = value;\n";
const UNRELATED: &str =
    "export function alone(): number { return 1; }\nexport const local = alone();\n";

type Mark = fn(&mut EntityDemand);

const FAMILIES: [(&str, Mark); 16] = [
    ("typeDescriptor", |d| d.type_descriptor = true),
    ("callability", |d| d.callability = true),
    ("constructability", |d| d.constructability = true),
    ("runtimeValueDomain", |d| d.runtime_value_domain = true),
    ("primitiveValueDomain", |d| d.primitive_value_domain = true),
    ("callResultDomain", |d| d.call_result_domain = true),
    ("constantValue", |d| d.constant_value = true),
    ("arrayShape", |d| d.array_shape = true),
    ("tupleShape", |d| d.tuple_shape = true),
    ("parameterObjectShape", |d| d.parameter_object_shape = true),
    ("runtimeIdentity", |d| d.runtime_identity = true),
    ("referenceSpace", |d| d.reference_space = true),
    ("references", |d| d.references = true),
    ("async", |d| d.r#async = true),
    ("structuralAccessor", |d| d.structural_accessor = true),
    ("libraryTypes", |d| d.library_types = true),
];

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
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "solid-typefacts-rs-delrestore-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(root.join("tsconfig.json"), CONFIG).unwrap();
        std::fs::write(root.join("dependency.ts"), DEPENDENCY).unwrap();
        std::fs::write(root.join("consumer.ts"), CONSUMER).unwrap();
        std::fs::write(root.join("unrelated.ts"), UNRELATED).unwrap();
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

/// Binding names and call callees, found by text so the demanded spans do not
/// depend on either producer's source facts.
fn spans(file: &str, source: &str) -> Vec<(String, usize, usize)> {
    let mut found = Vec::new();
    for needle in [
        "function value",
        "class Thing",
        "const list",
        "const pair",
        "const konst",
        "const result",
        "const made",
        "const l ",
        "const p ",
        "const k ",
        "const f ",
        "function alone",
        "const local",
    ] {
        if let Some(at) = source.find(needle) {
            let keyword = needle.split(' ').next().unwrap().len() + 1;
            let name = needle[keyword..].trim_end();
            found.push((file.to_owned(), at + keyword, at + keyword + name.len()));
        }
    }
    for callee in ["value()", "new Thing()", "alone()"] {
        if let Some(at) = source.find(callee) {
            let (offset, name) = callee
                .strip_prefix("new ")
                .map_or((0, callee), |rest| (4, rest));
            let name = name.trim_end_matches("()");
            found.push((file.to_owned(), at + offset, at + offset + name.len()));
        }
    }
    found
}

fn demands(project: &Project, files: &[(&str, &str)], mark: Mark) -> Vec<EntityDemand> {
    let mut demands: Vec<EntityDemand> = files
        .iter()
        .flat_map(|(file, source)| spans(file, source))
        .map(|(file, start, end)| {
            let mut demand = EntityDemand {
                location: Location {
                    path: project.path(&file).as_str().into(),
                    start_byte: start as u64,
                    end_byte: end as u64,
                },
                symbol: true,
                ..EntityDemand::default()
            };
            mark(&mut demand);
            demand
        })
        .collect();
    demands.sort_by(|left, right| {
        (&*left.location.path, left.location.start_byte)
            .cmp(&(&*right.location.path, right.location.start_byte))
    });
    demands
}

fn entities(table: &typefacts::FactTable) -> Vec<EntityFact> {
    let mut entities: Vec<EntityFact> = table.entities().cloned().collect();
    entities.sort_by(|left, right| {
        (
            &*left.location.path,
            left.location.start_byte,
            left.location.end_byte,
        )
            .cmp(&(
                &*right.location.path,
                right.location.start_byte,
                right.location.end_byte,
            ))
    });
    entities
}

/// Cold, after the deletion, after the restoration.
fn drive(binary: &Path, project: &Project, mark: Mark) -> Vec<Vec<EntityFact>> {
    let mut session = Session::open(
        Producer::at(binary),
        project.path("tsconfig.json"),
        Vec::<FileChange>::new(),
    )
    .expect("open");
    let all = [
        ("dependency.ts", DEPENDENCY),
        ("consumer.ts", CONSUMER),
        ("unrelated.ts", UNRELATED),
    ];
    let without_dependency = [("consumer.ts", CONSUMER), ("unrelated.ts", UNRELATED)];
    let mut analyses = Vec::new();
    let mut analyze = |session: &mut Session, files: &[(&str, &str)]| {
        let table = session
            .analyze(&AnalysisDemand {
                entities: demands(project, files, mark),
            })
            .expect("analyze");
        analyses.push(entities(&table));
    };
    analyze(&mut session, &all);
    session
        .update([FileChange {
            path: project.path("dependency.ts"),
            source: Vec::new(),
            deleted: true,
            version: 1,
        }])
        .expect("delete");
    analyze(&mut session, &without_dependency);
    session
        .update([FileChange {
            path: project.path("dependency.ts"),
            source: DEPENDENCY.as_bytes().to_vec(),
            deleted: false,
            version: 2,
        }])
        .expect("restore");
    analyze(&mut session, &all);
    let _ = session.close();
    analyses
}

/// The entities that differ between two analyses, as short Debug lines (a
/// whole table is too long to read in a failure).
fn difference(left: &[EntityFact], right: &[EntityFact]) -> Option<String> {
    if left == right {
        return None;
    }
    let short = |fact: &EntityFact| {
        let path = fact.location.path.rsplit('/').next().unwrap_or_default();
        format!(
            "{path}:{}-{} {fact:?}",
            fact.location.start_byte, fact.location.end_byte
        )
        .replace(&*fact.location.path, "")
    };
    let (left, right): (Vec<String>, Vec<String>) = (
        left.iter().map(short).collect(),
        right.iter().map(short).collect(),
    );
    let only_left: Vec<&String> = left.iter().filter(|line| !right.contains(line)).collect();
    let only_right: Vec<&String> = right.iter().filter(|line| !left.contains(line)).collect();
    Some(format!(
        "{} vs {} entities\nfirst only in left: {:#?}\nfirst only in right: {:#?}",
        left.len(),
        right.len(),
        &only_left[..only_left.len().min(3)],
        &only_right[..only_right.len().min(3)],
    ))
}

fn restored_equals_cold(who: &str, family: &str, analyses: &[Vec<EntityFact>]) {
    if let Some(difference) = difference(&analyses[0], &analyses[2]) {
        panic!("{who} [{family}]: the restored analysis differs from the cold one: {difference}");
    }
}

#[test]
fn delete_and_restore_answers_like_go_for_every_family() {
    let go = go_binary();
    if go.is_none() {
        eprintln!("NOTICE: no Go producer; checked against the cold analysis only");
    }
    let all_at_once: Mark = |d| {
        for (_, mark) in FAMILIES {
            mark(d);
        }
    };
    let mut runs: Vec<(&str, Mark)> = FAMILIES.to_vec();
    runs.push(("all", all_at_once));
    for (family, mark) in runs {
        let project = Project::new(family);
        let rust = drive(&rust_binary(), &project, mark);
        restored_equals_cold("rust", family, &rust);
        let Some(go) = &go else { continue };
        let go = drive(go, &project, mark);
        restored_equals_cold("go", family, &go);
        assert_eq!(go.len(), rust.len());
        for (phase, (go, rust)) in go.iter().zip(&rust).enumerate() {
            if let Some(difference) = difference(go, rust) {
                panic!("[{family}] phase {phase}: Go (left) and Rust (right) differ: {difference}");
            }
        }
    }
}
