//! The kept attestation (ADR 03XX-keep-attestation): after an update the
//! producer says replaced one file in place, the session keeps the previous
//! generation's attested imports and asks the producer about the edited file
//! alone; after every other kind of update it asks about every file.
//!
//! Each step compares what the session attested with what a session opened
//! fresh on the same bytes attests, so a kept index that differed from the
//! full one in any row fails here whether or not the producer says "kept".

use std::{env, fs, path::PathBuf, sync::Arc};

use solid_facts::{ProjectFacts, compiler::CompilerOptions};
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, dialect,
};

const STUB: &str = "declare module \"solid-js\" {\n  export function createSignal<T>(value: T): [() => T, (value: T) => void];\n}\n";

const CONFIG: &str = r#"{"compilerOptions":{"jsx":"react-jsx","jsxImportSource":"solid-js","module":"ESNext","moduleResolution":"Bundler","strict":true,"target":"ES2022"},"include":["*.ts","*.tsx"]}"#;

const FILES: [(&str, &str); 5] = [
    (
        "a.ts",
        "import { b } from \"./b\";\nexport const a = b + 1;\n",
    ),
    ("b.ts", "export const b = 1;\n"),
    ("c.ts", "export const c = 3;\n"),
    ("script.ts", "var script = 1;\n"),
    (
        "view.tsx",
        "import { createSignal } from \"solid-js\";\nexport const View = () => { const [n] = createSignal(1); return <div>{n()}</div>; };\n",
    ),
];

struct Scratch {
    root: PathBuf,
    typefacts: String,
    version: u64,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Scratch {
    /// `None` without a producer (the process tests skip silently, as the rest
    /// of this crate's do).
    fn new(tag: &str) -> Option<Self> {
        let typefacts = env::var("SOLID_TYPEFACTS_BIN").ok()?;
        let root = env::temp_dir()
            .join("sc-scratch.noindex")
            .join(format!("kept-attestation-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("solid-js.d.ts"), STUB).unwrap();
        fs::write(root.join("tsconfig.json"), CONFIG).unwrap();
        for (name, source) in FILES {
            fs::write(root.join(name), source).unwrap();
        }
        Some(Self {
            root: root.canonicalize().unwrap(),
            typefacts: PathBuf::from(typefacts)
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            version: 0,
        })
    }

    fn path(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().into_owned()
    }

    fn project_id(&self) -> String {
        self.path("tsconfig.json")
    }

    fn sources(&self) -> Vec<SourceFile> {
        FILES
            .iter()
            .map(|(name, _)| SourceFile {
                path: self.path(name),
                source: fs::read_to_string(self.root.join(name)).unwrap().into(),
                compiler_options: CompilerOptions::default(),
            })
            .collect()
    }

    /// A session over the project as it is on disk now, through `executable`
    /// (the producer itself or a wrapper that arms a crash).
    fn open(&self, executable: &str) -> NativeIncrementalSession {
        let typescript = TypeFactsSession::open(executable, &self.project_id(), &[]).unwrap();
        NativeIncrementalSession::open(
            dialect::by_id("solid-v2").expect("the 2.0 dialect is registered"),
            self.project_id(),
            self.sources(),
            typescript,
        )
        .unwrap()
    }

    /// Writes `edits` to disk (the producer reads them as overlays all the
    /// same) and sends them as one edit.
    fn edit(
        &mut self,
        session: &mut NativeIncrementalSession,
        edits: &[(&str, &str)],
    ) -> Arc<ProjectFacts> {
        self.version += 1;
        let changes = edits
            .iter()
            .map(|(name, source)| {
                fs::write(self.root.join(name), source).unwrap();
                SourceChange {
                    path: self.path(name),
                    version: self.version,
                    source: Some((*source).to_owned()),
                    compiler_options: CompilerOptions::default(),
                }
            })
            .collect();
        session.edit(changes, None).unwrap()
    }

    /// What a session opened fresh on the bytes now on disk attests.
    fn fresh(&self) -> Arc<ProjectFacts> {
        self.open(&self.typefacts).analyze().unwrap()
    }
}

fn assert_attests_like_a_fresh_session(scratch: &Scratch, facts: &ProjectFacts, label: &str) {
    let fresh = scratch.fresh();
    assert_eq!(
        facts.resolved_imports.as_ref().expect("attested"),
        fresh.resolved_imports.as_ref().expect("attested"),
        "{label}: the session's attested imports differ from a fresh session's"
    );
}

#[test]
fn a_file_replaced_in_place_is_the_only_file_asked_about_again() {
    let Some(mut scratch) = Scratch::new("in-place") else {
        return;
    };
    let mut session = scratch.open(&scratch.typefacts.clone());
    let first = session.analyze().unwrap();
    let all = first.files.len();
    let measurement = session.last_import_identity();
    assert!(!measurement.kept, "the first attestation is the full one");
    assert_eq!(measurement.requested, all);
    assert!(measurement.modules >= all);
    assert!(!session.update_kept_graph());

    // A body edit: the producer replaces b.ts in place, so only b.ts is asked.
    let facts = scratch.edit(&mut session, &[("b.ts", "export const b = 2;\n")]);
    assert!(session.update_kept_graph());
    let measurement = session.last_import_identity();
    assert!(measurement.kept);
    assert_eq!((measurement.requested, measurement.attested), (1, 1));
    assert_eq!(measurement.modules, 0, "no inventory is asked for");
    assert_attests_like_a_fresh_session(&scratch, &facts, "body edit");

    // The edited file's own rows move with its text: an edit that shifts its
    // import is still an in-place one (same specifier), and its new position
    // is the one attested.
    let facts = scratch.edit(
        &mut session,
        &[(
            "a.ts",
            "// moved\nimport { b } from \"./b\";\nexport const a = b + 2;\n",
        )],
    );
    assert!(session.last_import_identity().kept);
    assert_attests_like_a_fresh_session(&scratch, &facts, "a shifted import");
    let attested = facts.resolved_imports.as_ref().unwrap();
    let source = fs::read_to_string(scratch.root.join("a.ts")).unwrap();
    let start = source.find("\"./b\"").unwrap();
    let row = attested
        .iter()
        .find(|(path, row)| path.ends_with("a.ts") && &*row.text == "./b")
        .map(|(_, row)| row.span)
        .expect("a.ts's import is attested");
    assert_eq!(usize::try_from(row.start).unwrap(), start);

    // A JSX body edit under the same runtime import is kept as well, and a
    // second kept edit patches the patched index.
    let facts = scratch.edit(
        &mut session,
        &[(
            "view.tsx",
            "import { createSignal } from \"solid-js\";\nexport const View = () => { const [n] = createSignal(2); return <div>{n()}</div>; };\n",
        )],
    );
    assert!(session.last_import_identity().kept);
    assert_attests_like_a_fresh_session(&scratch, &facts, "a JSX body edit");
}

#[test]
fn every_edit_that_can_move_another_files_resolution_is_asked_about_in_full() {
    let Some(mut scratch) = Scratch::new("full") else {
        return;
    };
    let mut session = scratch.open(&scratch.typefacts.clone());
    session.analyze().unwrap();
    let all = session.last_import_identity().requested;
    let cases: [(&str, &str, &str); 7] = [
        (
            "an added import",
            "a.ts",
            "import { b } from \"./b\";\nimport { c } from \"./c\";\nexport const a = b + c;\n",
        ),
        (
            "the import removed again",
            "a.ts",
            "import { b } from \"./b\";\nexport const a = b + 1;\n",
        ),
        (
            "an added reference path directive",
            "a.ts",
            "/// <reference path=\"./script.ts\" />\nimport { b } from \"./b\";\nexport const a = b + 1;\n",
        ),
        (
            "an added reference types directive",
            "a.ts",
            "/// <reference types=\"solid-js\" />\nimport { b } from \"./b\";\nexport const a = b + 1;\n",
        ),
        (
            "an added JSX runtime pragma",
            "view.tsx",
            "/** @jsxImportSource preact */\nimport { createSignal } from \"solid-js\";\nexport const View = () => { const [n] = createSignal(1); return <div>{n()}</div>; };\n",
        ),
        (
            "an added global augmentation in a module",
            "b.ts",
            "export const b = 1;\ndeclare global { interface Window { w: number } }\n",
        ),
        (
            "an added ambient module in a script file",
            "script.ts",
            "var script = 1;\ndeclare module \"x\" { export const y: number; }\n",
        ),
    ];
    for (label, file, source) in cases {
        let facts = scratch.edit(&mut session, &[(file, source)]);
        let measurement = session.last_import_identity();
        assert!(
            !measurement.kept,
            "{label}: asked about only {measurement:?}"
        );
        assert_eq!(measurement.requested, all, "{label}");
        assert_attests_like_a_fresh_session(&scratch, &facts, label);
    }
    // A change to two files is never an in-place update.
    let facts = scratch.edit(
        &mut session,
        &[
            ("b.ts", "export const b = 5;\n"),
            ("c.ts", "export const c = 6;\n"),
        ],
    );
    assert!(!session.last_import_identity().kept);
    assert_attests_like_a_fresh_session(&scratch, &facts, "two files");
    // And the next body edit is kept again, from the full index the last
    // generation attested.
    let facts = scratch.edit(&mut session, &[("c.ts", "export const c = 7;\n")]);
    assert!(session.last_import_identity().kept);
    assert_attests_like_a_fresh_session(&scratch, &facts, "a body edit after a rebuilt chain");
}

/// A producer that restarted since the last attestation holds a program built
/// from the disk as it is now: nothing the earlier one answered is kept,
/// whether it died before the update landed or before the analysis.
#[cfg(unix)]
#[test]
fn a_restarted_producer_is_asked_about_every_file() {
    use std::os::unix::fs::PermissionsExt as _;
    for variable in [
        "SOLID_TYPEFACTS_CRASH_BEFORE_UPDATE",
        "SOLID_TYPEFACTS_CRASH_BEFORE_ANALYZE",
    ] {
        let Some(mut scratch) = Scratch::new(&variable.to_lowercase().replace('_', "-")) else {
            return;
        };
        let marker = scratch.root.join("crash-marker");
        let wrapper = scratch.root.join("producer.sh");
        fs::write(
            &wrapper,
            format!(
                "#!/bin/sh\n{variable}={marker} exec {typefacts} \"$@\"\n",
                marker = marker.display(),
                typefacts = scratch.typefacts,
            ),
        )
        .unwrap();
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
        let mut session = scratch.open(&wrapper.to_string_lossy());
        session.analyze().unwrap();
        let all = session.last_import_identity().requested;
        // Armed after the session is warm: the crash hits this edit.
        fs::write(&marker, b"armed").unwrap();
        let facts = scratch.edit(&mut session, &[("b.ts", "export const b = 2;\n")]);
        assert!(!marker.exists(), "{variable}: the crash was not consumed");
        let measurement = session.last_import_identity();
        assert!(!measurement.kept, "{variable}: {measurement:?}");
        assert_eq!(measurement.requested, all, "{variable}");
        assert_attests_like_a_fresh_session(&scratch, &facts, variable);
        // Later edits on the restarted producer are kept again.
        let facts = scratch.edit(&mut session, &[("b.ts", "export const b = 3;\n")]);
        assert!(session.last_import_identity().kept, "{variable}");
        assert_attests_like_a_fresh_session(&scratch, &facts, variable);
    }
}
