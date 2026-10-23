use std::path::Path;

use solid_facts::{
    FileFacts, ProjectFacts, TypeScriptTable,
    compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap},
    core::Generation,
};

use super::*;

fn facts(path: &str, source: &str) -> ProjectFacts {
    let ast = solid_facts::ast::extract(path, source).unwrap();
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
    let file = FileFacts::new(Generation::new(1).unwrap(), source, ast, compiler).unwrap();
    ProjectFacts {
        generation: Generation::new(1).unwrap(),
        project_id: "/app/tsconfig.json".into(),
        files: vec![file],
        typescript: TypeScriptTable::from_parts(
            3,
            1,
            "/app/tsconfig.json",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        typescript_changes: None,
        resolved_imports: None,
        runtime_resolutions: None,
        runtime_symbol_redirects: Default::default(),
        dependency_source_roots: Vec::new(),
    }
}

#[test]
fn every_runtime_load_form_is_one_site_of_its_kind() {
    let source = "\
import a from \"static-a\";
import type { T } from \"type-only\";
export { b } from \"reexport\";
export type { U } from \"type-reexport\";
import c = require(\"equals\");
const d = import(\"dynamic\");
const e = require(\"required\");
const f = import(computed());
";
    let sites = sites(&facts("/app/main.ts", source));
    let found: Vec<_> = sites
        .iter()
        .map(|site| (site.kind, site.text.as_str(), site.path.as_str()))
        .collect();
    // A type-only edge is erased and a computed specifier names nothing: no
    // row, so no answer, for either.
    assert_eq!(
        found,
        [
            ("static-import", "static-a", "/app/main.ts"),
            ("export-from", "reexport", "/app/main.ts"),
            ("import-equals", "equals", "/app/main.ts"),
            ("dynamic-import", "dynamic", "/app/main.ts"),
            ("require", "required", "/app/main.ts"),
        ]
    );
}

#[test]
fn the_request_names_a_development_serve_of_the_nearest_config() {
    let sites = sites(&facts(
        "/app/src/main.ts",
        "import a from \"a\";\nconst b = import(\"b\");\n",
    ));
    let request = request(Path::new("/app"), Path::new("/app/vite.config.ts"), &sites);
    assert_eq!(
        request,
        serde_json::json!({
            "protocol": "solid-checker-runtime-resolution-v1",
            "root": "/app",
            "configFile": "/app/vite.config.ts",
            // Not `--runtime-build`, not a target: the worker's client
            // dev-server answer is all this asks for.
            "mode": "development",
            "loads": [
                { "id": "0", "importer": "/app/src/main.ts", "specifier": "a", "kind": "static-import" },
                { "id": "1", "importer": "/app/src/main.ts", "specifier": "b", "kind": "dynamic-import" },
            ],
        })
    );
}

#[test]
fn the_governing_config_is_the_nearest_one_up_to_the_repository_root() {
    let directory = std::env::temp_dir().join(format!(
        "solid-checker-runtime-config-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    let write = |path: &str| {
        let path = directory.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "export default {};\n").unwrap();
    };
    write("repo/.git/HEAD");
    write("repo/vite.config.ts");
    write("repo/apps/web/src/.keep");
    write("repo/apps/own/vite.config.mjs");
    write("repo/apps/own/src/.keep");
    write("outside/vite.config.ts");
    write("outside/repo/.git/HEAD");
    write("outside/repo/app/.keep");
    // A monorepo app runs the root's config unless it has its own.
    assert_eq!(
        vite_config(&directory.join("repo/apps/web/src")),
        Some(directory.join("repo/vite.config.ts"))
    );
    assert_eq!(
        vite_config(&directory.join("repo/apps/own/src")),
        Some(directory.join("repo/apps/own/vite.config.mjs"))
    );
    // The walk stops at the repository root: a config above it governs
    // nothing, and no config means no answers at all.
    assert_eq!(vite_config(&directory.join("outside/repo/app")), None);
    std::fs::remove_dir_all(&directory).unwrap();
}
