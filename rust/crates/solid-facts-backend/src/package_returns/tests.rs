//! The entry selection's premise gate (review of ADR 0372): `cond-pkg`'s
//! `solid` and `default` entries return different keys, so a row says which
//! file the check took for the one that loads.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use solid_facts::compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap};
use solid_facts::core::Generation;
use solid_facts::runtime_resolution::{RuntimeOutcome, RuntimeResolutionIndex};
use solid_facts::{FileFacts, ProjectFacts, TypeScriptTable};
use solid_reactive_ir::{RuntimeBuild, RuntimeEnvironment, RuntimeTarget};

use super::package_return_keys;

const APP: &str = r#"import { attrs } from "cond-pkg";
export const C = () => <div {...attrs()} />;
"#;

/// A temporary project with `cond-pkg` installed, removed when dropped.
struct Project(PathBuf);

impl Project {
    fn new(plugin: bool) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "solid-checker-package-returns-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        let package = root.join("node_modules/cond-pkg");
        fs::create_dir_all(&package).unwrap();
        fs::write(root.join("package.json"), r#"{"name":"project"}"#).unwrap();
        fs::write(
            package.join("package.json"),
            r#"{"name":"cond-pkg","version":"0.0.0","type":"module","exports":{".":{"solid":"./solid.js","default":"./node.js"}}}"#,
        )
        .unwrap();
        fs::write(
            package.join("solid.js"),
            "export function attrs() { return { class: \"\" }; }\n",
        )
        .unwrap();
        fs::write(
            package.join("node.js"),
            "export function attrs() { return { class: \"\", onClick() {} }; }\n",
        )
        .unwrap();
        if plugin {
            let plugin = root.join("node_modules/@solidjs/vite-plugin");
            fs::create_dir_all(&plugin).unwrap();
            fs::write(
                plugin.join("package.json"),
                r#"{"name":"@solidjs/vite-plugin","version":"3.0.0-next.44"}"#,
            )
            .unwrap();
        }
        Self(fs::canonicalize(root).unwrap())
    }

    fn path(&self, relative: &str) -> String {
        self.0.join(relative).to_string_lossy().into_owned()
    }

    fn facts(&self, runtime_resolutions: Option<RuntimeResolutionIndex>) -> ProjectFacts {
        let path = self.path("App.tsx");
        let ast = solid_facts::ast::extract(path.as_str(), APP).unwrap();
        let compiler = ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: ast.source.hash.clone(),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        };
        let file = FileFacts::new(Generation::new(1).unwrap(), APP, ast, compiler).unwrap();
        let project_id = self.path("tsconfig.json");
        ProjectFacts {
            generation: Generation::new(1).unwrap(),
            project_id: project_id.clone(),
            files: vec![file],
            typescript: TypeScriptTable::from_parts(
                3,
                1,
                project_id,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            typescript_changes: None,
            resolved_imports: None,
            runtime_resolutions,
            runtime_symbol_redirects: Default::default(),
            dependency_source_roots: Vec::new(),
        }
    }

    /// The bundler's table answering the import with `entry`.
    fn bundler(&self, entry: &str) -> RuntimeResolutionIndex {
        let facts = self.facts(None);
        let import = &facts.files[0].ast.imports[0];
        let mut index = RuntimeResolutionIndex::default();
        let entry = self.path(&format!("node_modules/cond-pkg/{entry}"));
        index.insert(
            self.path("App.tsx"),
            import.span,
            import.module.as_str(),
            RuntimeOutcome::File {
                path: entry.as_str().into(),
                physical_path: entry.as_str().into(),
            },
        );
        index
    }

    /// The keys recorded for `attrs`, `None` when no row is.
    fn keys(
        &self,
        runtime_resolutions: Option<RuntimeResolutionIndex>,
        runtime: &RuntimeEnvironment,
    ) -> Option<Vec<String>> {
        let facts = self.facts(runtime_resolutions);
        let rows = package_return_keys(Path::new(&facts.project_id), &facts, runtime);
        rows.keys(&self.path("App.tsx"), "cond-pkg", "attrs")
            .map(|keys| keys.iter().cloned().collect())
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn host(target: RuntimeTarget) -> RuntimeEnvironment {
    RuntimeEnvironment {
        target: Some(target),
        build: Some(RuntimeBuild::Development),
        ..RuntimeEnvironment::default()
    }
}

#[test]
fn without_a_solid_vite_plugin_no_entry_is_read() {
    // The reviewer's case: a build without the plugin reads `default`, whose
    // `onClick` the `solid` entry does not return.
    let project = Project::new(false);
    assert_eq!(project.keys(None, &host(RuntimeTarget::Browser)), None);
}

#[test]
fn a_solid_vite_plugin_establishes_the_solid_entry() {
    let project = Project::new(true);
    assert_eq!(
        project.keys(None, &host(RuntimeTarget::Browser)),
        Some(vec!["class".to_owned()])
    );
    // No declared build selects nothing.
    let undeclared = RuntimeEnvironment {
        target: Some(RuntimeTarget::Browser),
        ..RuntimeEnvironment::default()
    };
    assert_eq!(project.keys(None, &undeclared), None);
}

#[test]
fn the_bundler_table_answers_alone_when_the_check_asked_it() {
    // The plugin is installed, but the bundler loads `node.js`: its answer,
    // not the conditions, is the entry.
    let project = Project::new(true);
    let browser = host(RuntimeTarget::Browser);
    assert_eq!(
        project.keys(Some(project.bundler("node.js")), &browser),
        Some(vec!["class".to_owned(), "onClick".to_owned()])
    );
    // A table without a row for the import withholds; so does one under a
    // host it does not answer (Vite's client development environment).
    assert_eq!(
        project.keys(Some(RuntimeResolutionIndex::default()), &browser),
        None
    );
    assert_eq!(
        project.keys(
            Some(project.bundler("solid.js")),
            &host(RuntimeTarget::Node)
        ),
        None
    );
    let production = RuntimeEnvironment {
        build: Some(RuntimeBuild::Production),
        ..browser.clone()
    };
    assert_eq!(
        project.keys(Some(project.bundler("solid.js")), &production),
        None
    );
    // And without the plugin the table still answers.
    let bare = Project::new(false);
    assert_eq!(
        bare.keys(Some(bare.bundler("solid.js")), &browser),
        Some(vec!["class".to_owned()])
    );
}
