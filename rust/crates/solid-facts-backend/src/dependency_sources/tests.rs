use std::collections::BTreeSet;

use super::{Host, entry_target, select};
use crate::host_constants::Ordered;
use solid_reactive_ir::package_boundary::{BindingForm, UnanalyzableReason};

fn ordered(json: &str) -> Ordered {
    serde_json::from_str(json).unwrap()
}

fn host(conditions: &[&str]) -> Host {
    Host {
        conditions: conditions.iter().map(|&c| c.to_owned()).collect(),
        browser: conditions.contains(&"browser"),
    }
}

#[test]
fn unsupported_runtime_bindings_stay_off_source_but_type_only_edges_do_not() {
    for source in [
        "export * as ns from 'pkg';",
        "import x = require('pkg'); export = x;",
        "const ns = await import('pkg'); ns.x();",
        "const ns = require('pkg'); ns.x();",
    ] {
        let facts = solid_facts::ast::extract("module.ts", source).unwrap();
        let bindings = super::unredirected_packages(&facts);
        assert!(bindings.contains("pkg"), "{source}");
        assert!(!bindings.contains("other"), "{source}");
    }
    // ADR 0313: a runtime namespace import keeps its own boundary
    // obligation and no longer takes the package off source.
    for source in [
        "import * as ns from 'pkg'; ns.x();",
        "export { x } from 'pkg';",
        "export { default as y } from 'pkg';",
        "export * from 'pkg';",
        "import { x } from 'pkg'; x();",
        "import x from 'pkg'; x();",
        "import { type T, x } from 'pkg'; x();",
        "import type * as ns from 'pkg'; let x: ns.T;",
        "export type { T } from 'pkg';",
        "export { type T } from 'pkg';",
        "export type * from 'pkg';",
        "import type x = require('pkg'); let y: x.T;",
        "type T = import('pkg').T;",
        "/** @type {import('pkg').T} */ let x;",
        "function require(x: string) { return x; } require('pkg');",
    ] {
        let facts = solid_facts::ast::extract("module.ts", source).unwrap();
        assert!(
            !super::unredirected_packages(&facts).contains("pkg"),
            "{source}"
        );
    }
    for source in [
        "const ns = await import(name);",
        "const ns = await import('./barrel');",
        "const ns = require(name);",
    ] {
        let facts = solid_facts::ast::extract("module.ts", source).unwrap();
        assert!(
            !super::unredirected_packages(&facts).contains("any-package"),
            "{source}"
        );
    }
}

#[test]
fn dialect_runtime_namespaces_do_not_retain_packages() {
    let bindings = super::UnsupportedBindings {
        runtime_packages: ["runtime".into()].into_iter().collect(),
        ..super::UnsupportedBindings::default()
    };
    for source in [
        "export * as ns from 'runtime';",
        "import * as ns from 'runtime'; ns.x();",
    ] {
        let facts = solid_facts::ast::extract("module.ts", source).unwrap();
        let mut bindings = bindings.clone();
        bindings.extend(super::unredirected_packages(&facts));
        assert!(!bindings.contains("runtime"), "{source}");
    }
}

#[test]
fn removing_a_package_removes_nested_roots_and_every_copy_of_their_names() {
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-source-pruning-{}",
        std::process::id()
    ));
    let root = |path: &str, name: &str| {
        let root = directory.join(path);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("package.json"), format!(r#"{{"name":"{name}"}}"#)).unwrap();
        std::fs::write(root.join("index.js"), "export const value = true;").unwrap();
        root
    };
    let parent = root("parent", "parent");
    let inner = root("parent/node_modules/inner", "inner");
    let peer = root("peer/node_modules/inner", "inner");
    let other = root("other", "other");
    for selected_name in ["parent", "alias"] {
        let roots = vec![parent.clone(), inner.clone(), peer.clone(), other.clone()];
        let selection = super::DependencySelection {
            files: roots.iter().map(|root| root.join("index.js")).collect(),
            resolutions: roots
                .iter()
                .zip([selected_name, "inner", "inner", "other"])
                .map(|(root, name)| super::ImportResolution {
                    importer: None,
                    specifier: name.into(),
                    target: root.join("index.js"),
                })
                .collect(),
            roots,
            package_names: [selected_name.into(), "inner".into(), "other".into()]
                .into_iter()
                .collect(),
        };
        let bindings = super::UnsupportedBindings {
            packages: [(
                selected_name.into(),
                (
                    UnanalyzableReason::UnsupportedBinding(BindingForm::NamespaceReexport),
                    "re-exported through a runtime binding".into(),
                ),
            )]
            .into_iter()
            .collect(),
            ..super::UnsupportedBindings::default()
        };
        assert_eq!(
            super::without_unredirected_packages(
                selection.clone(),
                &super::UnsupportedBindings::default(),
                &mut super::SelectionNotes::default()
            ),
            Some(selection.clone()),
            "an unrelated package remains selected",
        );
        assert!(
            super::without_unredirected_packages(
                selection.clone(),
                &super::UnsupportedBindings {
                    opaque: Some("a module load has no exact bare package target"),
                    ..super::UnsupportedBindings::default()
                },
                &mut super::SelectionNotes::default(),
            )
            .is_none()
        );
        let mut notes = super::SelectionNotes::default();
        let remaining =
            super::without_unredirected_packages(selection, &bindings, &mut notes).unwrap();
        // The decision behind each note is recorded beside it, by name.
        assert_eq!(notes.lines.len(), notes.refusals.len(), "{selected_name}");
        assert!(
            notes
                .refusals
                .iter()
                .any(|refusal| refusal.name == selected_name
                    && refusal.root.is_none()
                    && refusal.kind
                        == UnanalyzableReason::UnsupportedBinding(BindingForm::NamespaceReexport)
                    && refusal.detail == "re-exported through a runtime binding"),
            "{selected_name}: {notes:?}"
        );
        assert_eq!(
            remaining.roots,
            std::slice::from_ref(&other),
            "{selected_name}"
        );
        assert_eq!(remaining.files, [other.join("index.js")], "{selected_name}");
        assert_eq!(
            remaining.package_names,
            ["other".into()].into_iter().collect(),
            "{selected_name}"
        );
        assert_eq!(remaining.resolutions.len(), 1, "{selected_name}");
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_reexporting_package_without_manifest_identity_keeps_its_selection_identity() {
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-source-missing-identity-{}",
        std::process::id()
    ));
    let package = directory.join("node_modules/anonymous");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("package.json"), r#"{"module":"index.js"}"#).unwrap();
    std::fs::write(package.join("index.js"), "export { x } from 'other';").unwrap();
    let tsconfig = directory.join("tsconfig.json");
    std::fs::write(&tsconfig, "{}").unwrap();
    let mut notes = super::SelectionNotes::default();
    let prepared = super::prepare_program(
        &tsconfig,
        &["anonymous".into()],
        &host(&["browser", "development"]),
        &[],
        &[],
        &BTreeSet::new(),
        &[],
        &mut notes,
    )
    .unwrap();
    assert_eq!(
        prepared.unwrap().selection.package_names,
        ["anonymous".into()].into_iter().collect()
    );
    assert!(notes.lines.is_empty(), "{notes:?}");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_project_installing_none_of_the_packages_is_analyzed_as_usual() {
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-dependency-sources-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let tsconfig = directory.join("tsconfig.json");
    std::fs::write(&tsconfig, "{}").unwrap();
    let mut notes = super::SelectionNotes::default();
    let prepared = super::prepare_program(
        &tsconfig,
        &["not-installed-here".to_owned()],
        &host(&["browser", "development"]),
        &[],
        &[],
        &BTreeSet::new(),
        &[],
        &mut notes,
    )
    .unwrap();
    let written = std::fs::read_dir(&directory).unwrap().count();
    std::fs::remove_dir_all(&directory).unwrap();
    assert!(prepared.is_none());
    assert_eq!(written, 1, "no sibling tsconfig is written");
    assert_eq!(
        notes.lines,
        [
            "solid-checker: note: --dependency-source not-installed-here: not installed here, skipped"
        ]
    );
    assert_eq!(
        notes.refusals,
        [super::SourceRefusal {
            name: "not-installed-here".into(),
            root: None,
            kind: UnanalyzableReason::ResolutionUnavailable,
            detail: "not installed here".into(),
        }]
    );
}

#[test]
fn conditions_are_read_in_document_order() {
    let browser = host(&["browser", "development"]);
    let router =
        ordered(r#"{"exports":{".":{"solid":"./dist/index.jsx","default":"./dist/index.js"}}}"#);
    assert_eq!(
        entry_target(&router, ".", &browser).as_deref(),
        Some("./dist/index.jsx")
    );
    let tanstack = ordered(
        r#"{"exports":{".":{"solid":{"types":"./a.d.ts","default":"./dist/source/index.jsx"},"import":{"types":"./b.d.ts","default":"./dist/esm/index.js"}}}}"#,
    );
    assert_eq!(
        entry_target(&tanstack, ".", &browser).as_deref(),
        Some("./dist/source/index.jsx")
    );
    let node_first = ordered(r#"{"exports":{"node":"./node.js","import":"./browser.js"}}"#);
    assert_eq!(
        entry_target(&node_first, ".", &browser).as_deref(),
        Some("./browser.js")
    );
    let plain = ordered(r#"{"module":"./dist/index.js","main":"./dist/index.cjs"}"#);
    assert_eq!(
        entry_target(&plain, ".", &browser).as_deref(),
        Some("./dist/index.js")
    );
    assert!(select(&ordered(r#"{"require":"./a.cjs"}"#), &browser, 0, None).is_none());
}

#[test]
fn a_linked_workspace_package_is_project_code() {
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-dependency-workspace-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    let package = |root: &std::path::Path, name: &str| {
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
                root.join("package.json"),
                format!(
                    r#"{{"name":"{name}","peerDependencies":{{"solid-js":"2"}},"exports":{{".":{{"solid":"./src/index.js"}}}}}}"#
                ),
            )
            .unwrap();
        std::fs::write(root.join("src/index.js"), "export const value = 1;\n").unwrap();
    };
    package(&directory.join("packages/ui"), "ui");
    package(&directory.join("node_modules/real"), "real");
    std::fs::create_dir_all(directory.join("node_modules")).unwrap();
    std::os::unix::fs::symlink(
        directory.join("packages/ui"),
        directory.join("node_modules/ui"),
    )
    .unwrap();
    let app = directory.join("App.tsx");
    let mut notes = super::SelectionNotes::default();
    let prepared = super::prepare_all(
        vec![(app.clone(), "ui".into()), (app, "real".into())],
        &host(&["browser", "development"]),
        &["solid-js".to_owned()].into_iter().collect(),
        &["@solidjs/"],
        &mut super::UnsupportedBindings::default(),
        &mut notes,
    )
    .unwrap()
    .unwrap();
    let roots = prepared
        .roots
        .iter()
        .map(|root| root.file_name().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(roots, ["real"]);
    // Project code is no unanalyzable package: nothing is recorded for it.
    assert_eq!(notes, super::SelectionNotes::default());
}

#[test]
fn each_entry_refusal_names_its_decision() {
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-dependency-entry-refusals-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    let package = |name: &str, manifest: Option<&str>| {
        let root = directory.join(name);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("index.d.ts"), "export {};\n").unwrap();
        if let Some(manifest) = manifest {
            std::fs::write(root.join("package.json"), manifest).unwrap();
        }
        root.canonicalize().unwrap()
    };
    let browser = host(&["browser", "development"]);
    let kind = |root: &std::path::Path| super::select_entry(root, ".", &browser).unwrap_err().0;
    let unreadable = package("unreadable", None);
    let mapped = package(
        "mapped",
        Some(r#"{"browser":{"./a.js":"./b.js"},"module":"index.js"}"#),
    );
    let entryless = package("entryless", Some(r#"{"exports":{"./x":"./index.js"}}"#));
    let declaration = package("declaration", Some(r#"{"module":"index.d.ts"}"#));
    let missing = package("missing", Some(r#"{"module":"absent.js"}"#));
    let found = [
        kind(&unreadable),
        kind(&mapped),
        kind(&entryless),
        kind(&declaration),
        kind(&missing),
    ];
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(
        found,
        [
            UnanalyzableReason::ManifestUnavailable,
            UnanalyzableReason::RuntimeEntryUnavailable,
            UnanalyzableReason::RuntimeEntryUnavailable,
            UnanalyzableReason::RuntimeSourceUnavailable,
            UnanalyzableReason::RuntimeSourceUnavailable,
        ]
    );
}

#[test]
fn a_generated_program_is_written_outside_the_project_and_never_rewritten() {
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-dependency-sibling-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let program = |document: &[u8]| super::PreparedDependencyProgram {
        tsconfig: directory.join("tsconfig.json"),
        host: host(&["browser", "development"]),
        project_files: BTreeSet::new(),
        selection: super::DependencySelection {
            roots: Vec::new(),
            package_names: BTreeSet::new(),
            files: Vec::new(),
            resolutions: Vec::new(),
        },
        document: document.to_vec(),
    };
    let (logical, first) = program(b"{\"files\":[]}").write_retained(b"a").unwrap();
    // The logical path is beside the project's tsconfig, and nothing is
    // written there.
    assert_eq!(logical.parent(), Some(directory.as_path()));
    assert!(!logical.exists());
    assert!(!first.starts_with(&directory));
    assert_eq!(std::fs::read(&first).unwrap(), b"{\"files\":[]}");
    let stamp = std::fs::metadata(&first).unwrap().modified().unwrap();
    // The same bytes and domain reuse the file as it is.
    assert_eq!(
        program(b"{\"files\":[]}").write_retained(b"a").unwrap().1,
        first
    );
    assert_eq!(
        std::fs::metadata(&first).unwrap().modified().unwrap(),
        stamp
    );
    // Another domain or other bytes are another file.
    let other_domain = program(b"{\"files\":[]}").write_retained(b"b").unwrap().1;
    let other_bytes = program(b"{\"files\":[\"x\"]}")
        .write_retained(b"a")
        .unwrap()
        .1;
    assert_ne!(other_domain, first);
    assert_ne!(other_bytes, first);
    // A file of that name holding other bytes is refused, not rewritten.
    std::fs::write(&first, b"tampered").unwrap();
    assert!(program(b"{\"files\":[]}").write_retained(b"a").is_err());
    assert_eq!(std::fs::read(&first).unwrap(), b"tampered");
    for path in [&first, &other_domain, &other_bytes] {
        let _ = std::fs::remove_file(path);
    }
    // A one-shot check's program file goes with it.
    let generated = program(b"{}").write_for_process().unwrap();
    let config = generated.config().to_path_buf();
    assert!(config.is_file());
    assert!(!generated.path().exists());
    drop(generated);
    assert!(!config.exists());
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn every_runtime_imported_subpath_of_a_selected_package_is_selected() {
    let packages = ["@scope/pkg".to_owned()].into_iter().collect();
    let source = r#"
            import { a } from "@scope/pkg";
            import { b } from "@scope/pkg/sub";
            import type { C } from "@scope/pkg/types";
            import "@scope/pkg/styles.css";
            import { d } from "@scope/other";
            export { e } from "@scope/pkg/more";
            console.log(a, b, d);
        "#;
    let selected = super::imported_subpaths("/p/App.ts", source, &packages);
    assert_eq!(
        selected.into_iter().collect::<Vec<_>>(),
        ["@scope/pkg", "@scope/pkg/more", "@scope/pkg/sub"]
    );
}

#[test]
fn subpath_patterns_follow_node() {
    let browser = host(&["browser", "development"]);
    // Kobalte 2.0.0-alpha.0's map.
    let kobalte = ordered(
        r#"{"exports":{"./*":{"types":"./dist/*/index.d.ts","solid":"./dist/*/index.jsx","default":"./dist/*/index.js"}}}"#,
    );
    assert_eq!(
        entry_target(&kobalte, "./dialog", &browser).as_deref(),
        Some("./dist/dialog/index.jsx")
    );
    let nested = ordered(
        r#"{"exports":{"./*":"./dist/*.js","./icons/*":"./icons/*.js","./exact":"./exact.js"}}"#,
    );
    assert_eq!(
        entry_target(&nested, "./icons/a", &browser).as_deref(),
        Some("./icons/a.js")
    );
    assert_eq!(
        entry_target(&nested, "./exact", &browser).as_deref(),
        Some("./exact.js")
    );
    assert!(entry_target(&nested, "./../escape", &browser).is_none());
}

#[test]
fn the_declared_host_selects_the_entry() {
    // Review round 1: a fixed browser/development set read the source a
    // Node or production build never loads.
    let split = ordered(
        r#"{"exports":{"node":"./dist/node.js","development":{"solid":"./src/index.jsx"},"production":"./dist/prod.js","default":"./dist/index.js"}}"#,
    );
    assert_eq!(
        entry_target(&split, ".", &host(&["node", "development"])).as_deref(),
        Some("./dist/node.js")
    );
    assert_eq!(
        entry_target(&split, ".", &host(&["browser", "production"])).as_deref(),
        Some("./dist/prod.js")
    );
    assert_eq!(
        entry_target(&split, ".", &host(&["browser", "development"])).as_deref(),
        Some("./src/index.jsx")
    );
    // Without `exports`, a browser host reads `browser` before `module`,
    // and refuses a per-file `browser` map.
    let fields = ordered(r#"{"browser":"./browser.js","module":"./module.js","main":"./main.js"}"#);
    assert_eq!(
        entry_target(&fields, ".", &host(&["browser", "development"])).as_deref(),
        Some("./browser.js")
    );
    assert_eq!(
        entry_target(&fields, ".", &host(&["node", "development"])).as_deref(),
        Some("./module.js")
    );
    let remap = ordered(r#"{"browser":{"./a.js":"./b.js"},"module":"./module.js"}"#);
    assert!(entry_target(&remap, ".", &host(&["browser", "development"])).is_none());
    let empty = ordered(r#"{"browser":{},"module":"./module.js"}"#);
    assert_eq!(
        entry_target(&empty, ".", &host(&["browser", "development"])).as_deref(),
        Some("./module.js")
    );
}

#[test]
fn an_import_cycle_is_found_through_every_package() {
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-import-cycles-{}",
        std::process::id()
    ));
    let modules = directory.join("node_modules");
    let package = |name: &str, files: &[(&str, &str)]| {
        let root = modules.join(name);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("package.json"),
            format!(r#"{{"name":"{name}","exports":{{".":"./index.js"}}}}"#),
        )
        .unwrap();
        for (file, source) in files {
            std::fs::write(root.join(file), source).unwrap();
        }
        root.join("index.js")
    };
    let browser = host(&["browser", "development"]);
    let query = package(
        "query",
        &[(
            "index.js",
            "import { a } from \"core\";\nimport \"./local.js\";\nimport \"node:fs\";\nconst lazy = () => import(\"cyclic\");\n",
        )],
    );
    package(
        "core",
        &[
            ("index.js", "export * from \"./more.js\";\n"),
            ("more.js", "export const a = 1;\n"),
        ],
    );
    std::fs::write(modules.join("query/local.js"), "export {};\n").unwrap();
    assert!(super::outside_import_cycles(&query, &browser));
    // Back into the module from another package, two edges away.
    let cyclic = package("cyclic", &[("index.js", "import \"middle\";\n")]);
    package(
        "middle",
        &[("index.js", "export { useQuery } from \"cyclic\";\n")],
    );
    assert!(!super::outside_import_cycles(&cyclic, &browser));
    // Within one package.
    let local = package(
        "local",
        &[
            ("index.js", "import \"./a.js\";\n"),
            ("a.js", "import \"./index.js\";\n"),
        ],
    );
    assert!(!super::outside_import_cycles(&local, &browser));
    // An edge that cannot be followed may close a cycle.
    let missing = package("missing", &[("index.js", "import \"not-installed\";\n")]);
    assert!(!super::outside_import_cycles(&missing, &browser));
    let required = package(
        "required",
        &[("index.js", "const core = require(\"core\");\n")],
    );
    assert!(!super::outside_import_cycles(&required, &browser));
    let unread = package("unread", &[("index.js", "import \"./style.css\";\n")]);
    std::fs::write(modules.join("unread/style.css"), "").unwrap();
    assert!(!super::outside_import_cycles(&unread, &browser));
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn the_first_set_condition_decides_and_an_unread_value_refuses() {
    // Review round 1 of ADR 0295: Vite's resolver returns the first set
    // condition's value; an array there is not a reason to read `default`.
    let browser = host(&["browser", "development"]);
    for exports in [
        r#"{"browser":["./cycle.js"],"default":"./leaf.js"}"#,
        r#"{"browser":null,"default":"./leaf.js"}"#,
        r#"{"browser":{"worker":"./worker.js"},"default":"./leaf.js"}"#,
        r#"{".":{"import":["./a.js","./b.js"],"default":"./leaf.js"}}"#,
    ] {
        let manifest = ordered(&format!(r#"{{"exports":{exports}}}"#));
        assert!(
            entry_target(&manifest, ".", &browser).is_none(),
            "{exports}"
        );
    }
    // An unset condition's array is never read.
    let manifest = ordered(r#"{"exports":{"node":["./a.js"],"default":"./leaf.js"}}"#);
    assert_eq!(
        entry_target(&manifest, ".", &browser).as_deref(),
        Some("./leaf.js")
    );
}

#[test]
fn an_unread_exports_value_or_a_self_reference_does_not_hide_a_cycle() {
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-hidden-cycles-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    let write = |path: &str, contents: &str| {
        let path = directory.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    };
    let browser = host(&["browser", "development"]);
    // Finding 3: `bridge`'s browser array selects `cycle.js`, which
    // imports `p` back; `default` would have selected a leaf.
    write(
        "node_modules/p/package.json",
        r#"{"name":"p","exports":"./index.js"}"#,
    );
    write(
        "node_modules/p/index.js",
        "import { touch } from \"bridge\";\n",
    );
    write(
        "node_modules/bridge/package.json",
        r#"{"name":"bridge","exports":{"browser":["./cycle.js"],"default":"./leaf.js"}}"#,
    );
    write("node_modules/bridge/cycle.js", "import \"p\";\n");
    write("node_modules/bridge/leaf.js", "export {};\n");
    let p = directory.join("node_modules/p/index.js");
    assert!(!super::outside_import_cycles(&p, &browser));
    // The same package with a string the walk can follow: the cycle.
    write(
        "node_modules/bridge/package.json",
        r#"{"name":"bridge","exports":{"browser":"./cycle.js","default":"./leaf.js"}}"#,
    );
    assert!(!super::outside_import_cycles(&p, &browser));
    // And with the leaf: no cycle.
    write(
        "node_modules/bridge/package.json",
        r#"{"name":"bridge","exports":{"browser":"./leaf.js","default":"./cycle.js"}}"#,
    );
    assert!(super::outside_import_cycles(&p, &browser));
    // Finding 4: inside `self`, `import "self"` is `self` itself (it has
    // `exports`), not the nested copy.
    write(
        "node_modules/self/package.json",
        r#"{"name":"self","exports":"./index.js"}"#,
    );
    write(
        "node_modules/self/index.js",
        "import { a } from \"self\";\n",
    );
    write(
        "node_modules/self/node_modules/self/package.json",
        r#"{"name":"self","exports":"./index.js"}"#,
    );
    write(
        "node_modules/self/node_modules/self/index.js",
        "export const a = 1;\n",
    );
    let own = directory.join("node_modules/self/index.js");
    assert!(!super::outside_import_cycles(&own, &browser));
    assert_eq!(
        super::runtime_package_root(&own, "self"),
        Ok(Some(
            directory.join("node_modules/self").canonicalize().unwrap()
        ))
    );
    // Without `exports` there is no self-reference: Node's lookup finds
    // the nested copy.
    write(
        "node_modules/self/package.json",
        r#"{"name":"self","main":"./index.js"}"#,
    );
    assert!(super::outside_import_cycles(&own, &browser));
    // A nearer manifest of another name stops the search, and one this
    // cannot read refuses.
    write("node_modules/self/lib/package.json", r#"{"type":"module"}"#);
    write("node_modules/self/lib/inner.js", "import \"self\";\n");
    assert_eq!(
        super::runtime_package_root(&directory.join("node_modules/self/lib/inner.js"), "self"),
        Ok(Some(
            directory
                .join("node_modules/self/node_modules/self")
                .canonicalize()
                .unwrap()
        ))
    );
    write("node_modules/self/lib/package.json", "{");
    assert_eq!(
        super::runtime_package_root(&directory.join("node_modules/self/lib/inner.js"), "self"),
        Err(())
    );
    std::fs::remove_dir_all(&directory).unwrap();
}
