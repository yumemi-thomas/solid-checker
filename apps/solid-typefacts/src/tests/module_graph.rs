//! Ports `tsgo/module_graph_test.go` (and `project_path_test.go`, which lives
//! beside the code it tests in `module_graph.rs`) over the tsrs backend, with
//! the same projects and expectations.

use super::project_support::{BUNDLER_CONFIG, Scratch, relative};
use crate::backend::{ModuleGraph, ModuleInventory};
use crate::context::Context;
use crate::project::Project;
use typefacts::v3::ModuleGraphRequest;
use typefacts::{ModuleFact, ModuleFormat, ModuleImportFact, ModuleResolution};

fn graph(project: &mut Project, demand: &ModuleGraphRequest) -> ModuleInventory {
    project.module_graph(&Context::new(), demand).unwrap()
}

fn imports(imports: bool, packages: bool) -> ModuleGraphRequest {
    ModuleGraphRequest {
        imports,
        import_paths: Vec::new(),
        packages,
        distinct_bare_imports: false,
        omit_modules: false,
    }
}

fn module_paths(scratch: &Scratch, inventory: &ModuleInventory) -> Vec<String> {
    // Default library files live outside the project and are not part of what
    // these tests assert.
    let mut paths: Vec<String> = inventory
        .modules
        .iter()
        .filter_map(|module| relative(&scratch.root, &module.path))
        .collect();
    paths.sort();
    paths
}

fn import_by_text<'a>(inventory: &'a ModuleInventory, text: &str) -> &'a ModuleImportFact {
    inventory
        .imports
        .iter()
        .find(|fact| &*fact.text == text)
        .unwrap_or_else(|| panic!("no import fact for {text:?}; have {:?}", inventory.imports))
}

fn module_by_path<'a>(inventory: &'a ModuleInventory, path: &str) -> &'a ModuleFact {
    inventory
        .modules
        .iter()
        .find(|module| &*module.path == path)
        .unwrap_or_else(|| panic!("no module fact for {path:?}"))
}

/// ADR 0368: `root` is a file's position in the configuration's root files,
/// TypeScript's own order (here `files`, not path order); a file the program
/// reached through an import is not a root.
#[test]
fn module_facts_carry_the_configured_root_order() {
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"files":["main.ts","app.ts"]}"#,
        ),
        (
            "main.ts",
            "import { shared } from \"./shared\";\nexport const main = shared;\n",
        ),
        ("app.ts", "export const app = 1;\n"),
        ("shared.ts", "export const shared = 1;\n"),
    ]);
    let mut project = scratch.open();
    let inventory = graph(&mut project, &ModuleGraphRequest::default());
    let root = |name: &str| module_by_path(&inventory, &scratch.path(name)).root;
    assert_eq!(root("main.ts"), Some(0));
    assert_eq!(root("app.ts"), Some(1));
    assert_eq!(root("shared.ts"), None);
    assert!(
        inventory
            .modules
            .iter()
            .filter(|module| module.root.is_some())
            .count()
            == 2
    );
}

/// Review of ADR 0368: an extensionless `files` entry is a root too. The
/// loader loads `main.ts` for `"main"`; looking the configured name up
/// verbatim found no file and left both roots unplaced (path order).
#[test]
fn extensionless_configured_roots_keep_their_positions() {
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"files":["main","app"]}"#,
        ),
        ("main.ts", "export const main = 1;\n"),
        ("app.tsx", "export const app = 1;\n"),
    ]);
    let mut project = scratch.open();
    let inventory = graph(&mut project, &ModuleGraphRequest::default());
    let root = |name: &str| module_by_path(&inventory, &scratch.path(name)).root;
    assert_eq!(root("main.ts"), Some(0));
    assert_eq!(root("app.tsx"), Some(1));
    assert!(inventory.unmapped_roots.is_empty());
}

/// Review of ADR 0368: a root the program loads through a project-reference
/// redirect (a referenced project's declaration output, replaced by its
/// input) keeps its position on the file the program holds.
#[test]
fn a_redirected_project_reference_root_keeps_its_position() {
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{
  "compilerOptions": { "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler" },
  "files": ["app/index.ts", "lib/dist/channel.d.ts"],
  "references": [{ "path": "./lib" }]
}"#,
        ),
        (
            "lib/tsconfig.json",
            r#"{
  "compilerOptions": {
    "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler",
    "composite": true, "declaration": true, "outDir": "./dist", "rootDir": "./src"
  },
  "include": ["src/**/*.ts"]
}"#,
        ),
        (
            "lib/src/channel.ts",
            "export function channelFor() { return 1; }\n",
        ),
        (
            "lib/dist/channel.d.ts",
            "export declare function channelFor(): number;\n",
        ),
        ("app/index.ts", "export const app = 1;\n"),
    ]);
    let mut project = scratch.open();
    let inventory = graph(&mut project, &ModuleGraphRequest::default());
    let root = |name: &str| module_by_path(&inventory, &scratch.path(name)).root;
    assert_eq!(root("app/index.ts"), Some(0));
    assert_eq!(root("lib/src/channel.ts"), Some(1));
    assert!(inventory.unmapped_roots.is_empty());
}

#[test]
fn module_inventory_reports_every_included_file() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", BUNDLER_CONFIG),
        (
            "index.ts",
            "import { helper } from \"./helper\";\nimport type { Shape } from \"./shape\";\nexport const value: Shape = helper();\n",
        ),
        (
            "helper.ts",
            "import type { Shape } from \"./shape\";\nexport function helper(): Shape { return { tag: \"shape\" }; }\n",
        ),
        ("shape.d.ts", "export interface Shape { tag: string }\n"),
        ("unreached.ts", "export const unreached = 1;\n"),
    ]);
    let mut project = scratch.open();
    let inventory = graph(&mut project, &ModuleGraphRequest::default());
    assert_eq!(
        module_paths(&scratch, &inventory),
        ["helper.ts", "index.ts", "shape.d.ts", "unreached.ts"]
    );
    assert!(module_by_path(&inventory, &scratch.path("shape.d.ts")).declaration_file);
    let index = module_by_path(&inventory, &scratch.path("index.ts"));
    assert!(!index.declaration_file);
    assert_eq!(index.format, ModuleFormat::Esm);
    // The inventory is the program's own file list, so the default library
    // files it opened are in it too. That is the point: a closure record built
    // from it names what the analysis read rather than what a scanner guessed.
    assert!(inventory.modules.len() > 4, "{}", inventory.modules.len());
    assert!(inventory.imports.is_empty(), "no import demand, no imports");
    // The inventory is path-ordered.
    assert!(
        inventory
            .modules
            .windows(2)
            .all(|pair| pair[0].path < pair[1].path)
    );
}

#[test]
fn module_graph_reports_relative_import_resolution() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", BUNDLER_CONFIG),
        (
            "index.ts",
            "import { helper } from \"./nested/helper\";\nexport const value = helper();\n",
        ),
        (
            "nested/helper.ts",
            "export function helper() { return 1; }\n",
        ),
    ]);
    let mut project = scratch.open();
    let inventory = graph(
        &mut project,
        &ModuleGraphRequest {
            imports: true,
            import_paths: vec![scratch.path("index.ts")],
            packages: true,
            distinct_bare_imports: false,
            omit_modules: false,
        },
    );
    assert_eq!(inventory.imports.len(), 1);
    let fact = &inventory.imports[0];
    assert_eq!(fact.resolution, ModuleResolution::Relative);
    assert_eq!(&*fact.resolved_path, scratch.path("nested/helper.ts"));
    assert_eq!(&*fact.extension, ".ts");
    assert_eq!(&*fact.symlink_path, "");
    assert_eq!(&*fact.paths_pattern, "");
    assert!(fact.resolver_package.is_none(), "relative import");
    // A project with no manifest of its own answers no package identity.
    assert!(fact.package.is_none(), "{:?}", fact.package);
    let source = std::fs::read(scratch.path("index.ts")).unwrap();
    let span = &source[fact.specifier.start_byte as usize..fact.specifier.end_byte as usize];
    assert_eq!(span, b"\"./nested/helper\"");
}

/// `distinctBareImports` (ADR 0382): what resolution read, once per importing
/// file and resolution. Relative imports and a repeated `import("pkg")` type
/// add nothing a resolution witness observes, so only the first `pkg` row and
/// the distinct `pkg/sub` row are answered, with the full demand's values.
#[test]
fn distinct_bare_imports_keep_one_row_per_resolution() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", BUNDLER_CONFIG),
        (
            "index.ts",
            "import { a } from \"pkg\";\nimport \"./local\";\nexport type T = import(\"pkg\").A;\nexport type U = import(\"pkg\").A;\nexport type V = import(\"pkg/sub\").B;\nexport const value = a;\n",
        ),
        ("local.ts", "export {};\n"),
        (
            "node_modules/pkg/package.json",
            r#"{"name":"pkg","types":"index.d.ts"}"#,
        ),
        (
            "node_modules/pkg/index.d.ts",
            "export declare const a: number;\nexport interface A {}\n",
        ),
        ("node_modules/pkg/sub.d.ts", "export interface B {}\n"),
    ]);
    let mut project = scratch.open();
    let index = scratch.path("index.ts");
    let full = graph(&mut project, &imports(true, false));
    let distinct = graph(
        &mut project,
        &ModuleGraphRequest {
            imports: true,
            import_paths: Vec::new(),
            packages: false,
            distinct_bare_imports: true,
            omit_modules: false,
        },
    );
    let of = |inventory: &ModuleInventory| {
        inventory
            .imports
            .iter()
            .filter(|fact| *fact.specifier.path == index)
            .cloned()
            .collect::<Vec<_>>()
    };
    let full = of(&full);
    assert_eq!(full.len(), 5, "{full:?}");
    let texts = of(&distinct)
        .iter()
        .map(|fact| fact.text.to_string())
        .collect::<Vec<_>>();
    assert_eq!(texts, ["pkg", "pkg/sub"]);
    let first = of(&distinct).into_iter().next().unwrap();
    assert_eq!(
        first, full[0],
        "the first occurrence, as the full demand answers it"
    );
    assert!(!first.resolved_path.is_empty(), "{first:?}");
}

// A declaration file beside a runtime file is the shape almost every published
// package has, and it is the case the protocol cannot close. The compiler
// resolves the specifier to the declaration file, includes the runtime file as
// an unrelated root, and records nothing joining the two.
#[test]
fn declaration_sibling_has_no_compiler_recorded_runtime_pairing() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", BUNDLER_CONFIG),
        (
            "index.ts",
            "import { channelFor } from \"./channel.js\";\nexport const channel = channelFor();\n",
        ),
        (
            "channel.js",
            "export function channelFor() { return { live: true }; }\n",
        ),
        (
            "channel.d.ts",
            "export declare function channelFor(): { live: boolean };\n",
        ),
    ]);
    let mut project = scratch.open();
    let inventory = graph(&mut project, &imports(true, false));
    let fact = import_by_text(&inventory, "./channel.js");
    let declaration = scratch.path("channel.d.ts");
    assert_eq!(&*fact.resolved_path, declaration);
    assert_eq!(&*fact.extension, ".d.ts");
    // Both files are in the program, as unrelated roots.
    let runtime_module = module_by_path(&inventory, &scratch.path("channel.js"));
    let declaration_module = module_by_path(&inventory, &declaration);
    assert!(declaration_module.declaration_file && !runtime_module.declaration_file);
    // And nothing pairs them.
    assert!(declaration_module.project_reference.is_none());
    assert!(declaration_module.redirect_targets.is_empty());
    assert_eq!(&*fact.included_path, "");
}

// The one declaration-to-input pairing TypeScript does record is a configured
// project reference's declaration output.
#[test]
fn project_reference_declaration_output_carries_its_input() {
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{
  "compilerOptions": { "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler" },
  "include": ["app/**/*.ts"],
  "references": [{ "path": "./lib" }]
}"#,
        ),
        (
            "lib/tsconfig.json",
            r#"{
  "compilerOptions": {
    "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler",
    "composite": true, "declaration": true, "outDir": "./dist", "rootDir": "./src"
  },
  "include": ["src/**/*.ts"]
}"#,
        ),
        (
            "lib/src/channel.ts",
            "export function channelFor() { return { live: true }; }\n",
        ),
        (
            "lib/dist/channel.d.ts",
            "export declare function channelFor(): { live: boolean };\n",
        ),
        (
            "lib/dist/channel.js",
            "export function channelFor() { return { live: true }; }\n",
        ),
        (
            "app/index.ts",
            "import { channelFor } from \"../lib/dist/channel.js\";\nexport const channel = channelFor();\n",
        ),
    ]);
    let mut project = scratch.open();
    let inventory = graph(&mut project, &imports(true, false));
    let fact = import_by_text(&inventory, "../lib/dist/channel.js");
    assert_eq!(fact.resolution, ModuleResolution::Relative);
    let source = scratch.path("lib/src/channel.ts");
    let declaration = scratch.path("lib/dist/channel.d.ts");
    // The resolver still selects the declaration output; the program redirects
    // it to the input it was emitted from.
    assert_eq!(&*fact.resolved_path, declaration);
    assert_eq!(&*fact.included_path, source);
    let module = module_by_path(&inventory, &source);
    let mapping = module
        .project_reference
        .as_ref()
        .expect("project reference");
    assert_eq!(&*mapping.source, source);
    assert_eq!(&*mapping.output_dts, declaration);
    assert!(
        inventory
            .modules
            .iter()
            .all(|other| *other.path != declaration),
        "the redirected declaration output is in the inventory as its own module"
    );
}

#[test]
fn paths_aliased_import_is_distinguishable_from_the_installed_package() {
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{
  "compilerOptions": {
    "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler",
    "paths": { "reactive-package": ["./src/local-impl.ts"], "shared/*": ["./src/shared/*"] }
  },
  "include": ["src/**/*.ts"]
}"#,
        ),
        (
            "src/local-impl.ts",
            "export function createReactive() { return 1; }\n",
        ),
        ("src/shared/util.ts", "export const util = 1;\n"),
        (
            "src/index.ts",
            "import { createReactive } from \"reactive-package\";\nimport { util } from \"shared/util\";\nexport const value = createReactive() + util;\n",
        ),
        (
            "node_modules/reactive-package/package.json",
            r#"{"name":"reactive-package","version":"4.2.0","main":"index.js","types":"index.d.ts"}"#,
        ),
        (
            "node_modules/reactive-package/index.d.ts",
            "export declare function createReactive(): number;\n",
        ),
        (
            "node_modules/reactive-package/index.js",
            "export function createReactive() { return 2; }\n",
        ),
    ]);
    let mut project = scratch.open();
    let inventory = graph(&mut project, &imports(true, true));
    let aliased = import_by_text(&inventory, "reactive-package");
    assert_eq!(&*aliased.resolved_path, scratch.path("src/local-impl.ts"));
    // The decisive pair: a bare specifier that a `paths` key matched and that
    // did not land in node_modules is not the installed package of that name,
    // however identical the specifier text is.
    assert_eq!(aliased.resolution, ModuleResolution::NonRelative);
    assert_eq!(&*aliased.paths_pattern, "reactive-package");
    assert!(
        aliased.resolver_package.is_none(),
        "no manifest was consulted"
    );
    assert!(
        aliased
            .package
            .as_ref()
            .is_none_or(|package| &*package.name != "reactive-package")
    );
    assert_eq!(
        &*import_by_text(&inventory, "shared/util").paths_pattern,
        "shared/*"
    );
}

#[test]
fn node_modules_import_carries_owning_package_identity() {
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{
  "compilerOptions": { "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler" },
  "include": ["src/**/*.ts"]
}"#,
        ),
        (
            "package.json",
            r#"{"name":"host-project","version":"0.1.0"}"#,
        ),
        (
            "src/index.ts",
            "import { createReactive } from \"reactive-package\";\nimport { deep } from \"reactive-package/deep\";\nexport const value = createReactive() + deep;\n",
        ),
        (
            "node_modules/reactive-package/package.json",
            r#"{"name":"reactive-package","version":"4.2.0","exports":{".":"./index.js","./deep":"./nested/deep.js"}}"#,
        ),
        (
            "node_modules/reactive-package/index.d.ts",
            "export declare function createReactive(): number;\n",
        ),
        (
            "node_modules/reactive-package/index.js",
            "export function createReactive() { return 2; }\n",
        ),
        (
            "node_modules/reactive-package/nested/deep.d.ts",
            "export declare const deep: number;\n",
        ),
        (
            "node_modules/reactive-package/nested/deep.js",
            "export const deep = 3;\n",
        ),
    ]);
    let mut project = scratch.open();
    let inventory = graph(&mut project, &imports(true, true));
    let root = import_by_text(&inventory, "reactive-package");
    assert_eq!(root.resolution, ModuleResolution::NodeModules);
    let package = root.package.as_ref().expect("the owning manifest");
    let manifest = scratch.path("node_modules/reactive-package/package.json");
    assert_eq!(&*package.manifest_path, manifest);
    assert_eq!(
        (&*package.name, &*package.version),
        ("reactive-package", "4.2.0")
    );
    let resolver = root.resolver_package.as_ref().expect("resolver package");
    assert_eq!(
        (&*resolver.name, &*resolver.version),
        ("reactive-package", "4.2.0")
    );
    let deep = import_by_text(&inventory, "reactive-package/deep");
    assert_eq!(
        &*deep.package.as_ref().expect("package").manifest_path,
        manifest
    );
    // The sub-module name is the path of the file the resolver landed on
    // within the package, not the `exports` key that selected it.
    assert_eq!(
        &*deep.resolver_package.as_ref().expect("resolver").subpath,
        "nested/deep.d.ts"
    );
}

// pnpm installs a package once in a content-addressed store and links it into
// each dependent's node_modules. TypeScript resolves through the link and then
// takes the realpath, so the two paths differ and both are reported.
#[cfg(unix)]
#[test]
fn symlinked_package_reports_both_paths() {
    let store = "node_modules/.store/reactive-package@4.2.0/node_modules/reactive-package";
    let scratch = Scratch::new(&[
        (
            "tsconfig.json",
            r#"{
  "compilerOptions": { "strict": true, "module": "esnext", "target": "esnext", "moduleResolution": "bundler" },
  "include": ["src/**/*.ts"]
}"#,
        ),
        (
            "src/index.ts",
            "import { createReactive } from \"reactive-package\";\nexport const value = createReactive();\n",
        ),
        (
            &format!("{store}/package.json"),
            r#"{"name":"reactive-package","version":"4.2.0","main":"index.js","types":"index.d.ts"}"#,
        ),
        (
            &format!("{store}/index.d.ts"),
            "export declare function createReactive(): number;\n",
        ),
        (
            &format!("{store}/index.js"),
            "export function createReactive() { return 2; }\n",
        ),
    ]);
    let link = scratch.path("node_modules/reactive-package");
    let target = scratch.path(store);
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let mut project = scratch.open();
    let inventory = graph(&mut project, &imports(true, true));
    let fact = import_by_text(&inventory, "reactive-package");
    assert_eq!(fact.resolution, ModuleResolution::NodeModules);
    assert_eq!(
        &*fact.resolved_path,
        format!("{target}/index.d.ts"),
        "the realpath"
    );
    assert_eq!(
        &*fact.symlink_path,
        format!("{link}/index.d.ts"),
        "the as-resolved link path"
    );
    // The owning manifest is looked up from the realpath, so it is the store
    // copy's manifest rather than the link's.
    assert_eq!(
        &*fact.package.as_ref().expect("package").manifest_path,
        format!("{target}/package.json")
    );
    // The inventory names the realpath too.
    module_by_path(&inventory, &format!("{target}/index.d.ts"));
}

#[test]
fn unknown_import_paths_are_reported_rather_than_dropped() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", BUNDLER_CONFIG),
        ("index.ts", "export const value = 1;\n"),
    ]);
    let mut project = scratch.open();
    let missing = scratch.path("absent.ts");
    let inventory = graph(
        &mut project,
        &ModuleGraphRequest {
            imports: true,
            import_paths: vec![scratch.path("index.ts"), missing.clone(), missing.clone()],
            packages: false,
            distinct_bare_imports: false,
            omit_modules: false,
        },
    );
    assert!(inventory.imports.is_empty(), "{:?}", inventory.imports);
    assert_eq!(
        inventory
            .unknown_import_paths
            .iter()
            .map(|path| path.to_string())
            .collect::<Vec<_>>(),
        [missing]
    );
}

#[test]
fn unresolved_specifier_is_reported_as_unresolved() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", BUNDLER_CONFIG),
        (
            "index.ts",
            "// @ts-expect-error missing package\nimport { nothing } from \"never-installed\";\nexport const value = nothing;\n",
        ),
    ]);
    let mut project = scratch.open();
    let inventory = graph(&mut project, &imports(true, false));
    let fact = import_by_text(&inventory, "never-installed");
    assert_eq!(fact.resolution, ModuleResolution::Unresolved);
    assert_eq!(&*fact.resolved_path, "");
}

#[test]
fn a_cancelled_request_and_a_closed_project_fail_instead_of_answering() {
    use crate::backend::ClosureBackend;
    let scratch = Scratch::new(&[
        ("tsconfig.json", BUNDLER_CONFIG),
        ("index.ts", "export const value = 1;\n"),
    ]);
    let mut project = scratch.open();
    let ctx = Context::new();
    ctx.cancel();
    let error = project
        .module_graph(&ctx, &ModuleGraphRequest::default())
        .unwrap_err();
    assert_eq!(error.0, "context canceled");
    project.close().unwrap();
    let error = project
        .module_graph(&Context::new(), &ModuleGraphRequest::default())
        .unwrap_err();
    assert_eq!(error.0, crate::project::ERR_CLOSED);
    assert_eq!(project.close().unwrap_err().0, crate::project::ERR_CLOSED);
}

/// `omitModules`: the answer carries no inventory (no modules, no unmapped
/// roots), and the import rows of the requested files are the ones the full
/// answer holds.
#[test]
fn omit_modules_answers_the_requested_imports_alone() {
    let scratch = Scratch::new(&[
        ("tsconfig.json", BUNDLER_CONFIG),
        (
            "index.ts",
            "import { helper } from \"./nested/helper\";\nexport const value = helper();\n",
        ),
        (
            "nested/helper.ts",
            "export function helper() { return 1; }\n",
        ),
        (
            "other.ts",
            "import { value } from \"./index\";\nexport const again = value;\n",
        ),
    ]);
    let mut project = scratch.open();
    let demand = |omit_modules| ModuleGraphRequest {
        imports: true,
        import_paths: vec![scratch.path("index.ts")],
        packages: true,
        distinct_bare_imports: false,
        omit_modules,
    };
    let full = graph(&mut project, &demand(false));
    let narrow = graph(&mut project, &demand(true));
    assert!(!full.modules.is_empty());
    assert!(narrow.modules.is_empty(), "{:?}", narrow.modules);
    assert!(narrow.unmapped_roots.is_empty());
    assert_eq!(narrow.imports, full.imports);
    assert_eq!(narrow.unknown_import_paths, full.unknown_import_paths);
    // A path the program does not hold is still reported, never dropped.
    let missing = graph(
        &mut project,
        &ModuleGraphRequest {
            import_paths: vec![scratch.path("missing.ts")],
            ..demand(true)
        },
    );
    assert!(missing.modules.is_empty());
    assert_eq!(
        missing
            .unknown_import_paths
            .iter()
            .map(|path| path.to_string())
            .collect::<Vec<_>>(),
        [scratch.path("missing.ts")]
    );
}
