use super::*;

const SCOPE: GapScope = GapScope {
    specifier: "solid-js",
    exports: &[
        "createErrorBoundary",
        "createLoadingBoundary",
        "createRevealOrder",
        "sharedConfig",
        "$DEVCOMP",
    ],
};

fn reaches(files: &[(&str, &str)]) -> Vec<Reach> {
    let facts = files
        .iter()
        .map(|(path, source)| {
            (
                *path,
                *source,
                solid_facts::ast::extract(*path, source).expect("the source parses"),
            )
        })
        .collect::<Vec<_>>();
    scope_sites(
        &SCOPE,
        facts
            .iter()
            .map(|(path, source, ast)| (*path, *source, ast)),
        None,
    )
    .into_iter()
    .map(|site| site.reach)
    .collect()
}

fn one(source: &str) -> Vec<Reach> {
    reaches(&[("/p/App.tsx", source)])
}

#[test]
fn a_project_that_names_none_of_the_exports_reaches_nothing() {
    assert_eq!(
        one(r#"import { createSignal, createMemo } from "solid-js";
                   import "solid-js";
                   import { createErrorBoundary } from "solid-js/internal";
                   import { createErrorBoundary as other } from "./local";
                   export { createErrorBoundary } from "some-other-package";
                   const [a] = createSignal(1);
                   const b = createMemo(() => a());
                   const load = (name: string) => import(name);"#),
        []
    );
}

#[test]
fn every_import_form_that_names_an_export_reaches_it() {
    // Named, aliased, type-only, and a string-literal import name.
    assert_eq!(
        one(
            r#"import { createErrorBoundary, sharedConfig as config } from "solid-js";
                   import type { createRevealOrder } from "solid-js";
                   import { "$DEVCOMP" as brand } from "solid-js";"#
        ),
        [
            Reach::Export("createErrorBoundary"),
            Reach::Export("sharedConfig"),
            Reach::Export("createRevealOrder"),
            Reach::Export("$DEVCOMP"),
        ]
    );
    // A project module re-exporting one, aliased or not.
    assert_eq!(
        one(r#"export { createLoadingBoundary as Loading, createSignal } from "solid-js";"#),
        [Reach::Export("createLoadingBoundary")]
    );
}

#[test]
fn a_namespace_import_reaches_only_the_members_it_names() {
    assert_eq!(
        one(r#"import * as S from "solid-js";
                   const [a] = S.createSignal(1);
                   S.createErrorBoundary(() => a(), () => null);
                   const view = <S.Show when={a()}>x</S.Show>;"#),
        [Reach::Export("createErrorBoundary")]
    );
    // A literal computed key names exactly one export; `S[0]` names none.
    // A transparent TypeScript wrapper does not change the literal key.
    assert_eq!(
        one(r#"import * as S from "solid-js";
                   const a = S["sharedConfig"];
                   const b = S[0];"#),
        [Reach::Export("sharedConfig")]
    );
    assert_eq!(
        one(r#"import * as S from "solid-js";
                   const a = S["sharedConfig" as const];"#),
        [Reach::Export("sharedConfig")]
    );
    // Escaping into a call, a destructuring, a re-export, or a type.
    for escape in [
        "consume(S);",
        "const { createErrorBoundary } = S;",
        "export { S };",
        "type T = typeof S;",
        "declare const key: string; const k = S[key];",
    ] {
        assert_eq!(
            one(&format!("import * as S from \"solid-js\";\n{escape}")),
            [Reach::Unbounded(Unbounded::NamespaceEscapes)],
            "{escape}"
        );
    }
}

#[test]
fn uses_that_name_no_export_keep_the_gap_open() {
    assert_eq!(
        one(r#"import Solid from "solid-js";"#),
        [Reach::Unbounded(Unbounded::DefaultImport)]
    );
    assert_eq!(
        one(r#"export * from "solid-js";"#),
        [Reach::Unbounded(Unbounded::ExportStar)]
    );
    assert_eq!(
        one(r#"export * as Solid from "solid-js";"#),
        [Reach::Unbounded(Unbounded::NamespaceReExport)]
    );
    assert_eq!(
        one(r#"const solid = await import("solid-js");"#),
        [Reach::Unbounded(Unbounded::ModuleLoad)]
    );
}

#[test]
fn a_shadowed_namespace_name_is_not_the_import() {
    // The inner `S` is a parameter: its member read is not the namespace's.
    assert_eq!(
        one(r#"import * as S from "solid-js";
                   const f = (S: { createErrorBoundary(): void }) => S.createErrorBoundary();
                   S.createSignal(1);"#),
        []
    );
}

#[test]
fn a_re_export_through_a_project_module_is_seen_in_the_module_that_re_exports() {
    assert_eq!(
        reaches(&[
            (
                "/p/boundary.ts",
                r#"export { createErrorBoundary } from "solid-js";"#
            ),
            (
                "/p/App.tsx",
                r#"import { createErrorBoundary } from "./boundary";
                       createErrorBoundary(() => 1, () => 2);"#
            ),
        ]),
        [Reach::Export("createErrorBoundary")]
    );
}

#[test]
fn an_import_equals_namespace_is_read_like_a_namespace_import() {
    assert_eq!(
        one(r#"import S = require("solid-js");
                   S.createSignal(1);
                   S.createErrorBoundary(() => 1, () => 2);"#),
        [Reach::Export("createErrorBoundary")]
    );
    assert_eq!(
        one(r#"import S = require("solid-js");
                   S.createSignal(1);"#),
        []
    );
    assert_eq!(
        one(r#"import S = require("solid-js");
                   consume(S);"#),
        [Reach::Unbounded(Unbounded::NamespaceEscapes)]
    );
    assert_eq!(
        one(r#"export import S = require("solid-js");"#),
        [Reach::Unbounded(Unbounded::ExportedImportEquals)]
    );
    // Another module's import-equals reaches nothing here.
    assert_eq!(
        one(r#"import S = require("./local");
                   S.createErrorBoundary(() => 1, () => 2);"#),
        []
    );
}

#[test]
fn a_type_position_import_is_counted_like_a_type_only_import() {
    assert_eq!(
        one(r#"type A = typeof import("solid-js").createErrorBoundary;
                   type B = import("solid-js").Accessor<number>;"#),
        [Reach::Export("createErrorBoundary")]
    );
    assert_eq!(
        one(r#"type N = typeof import("solid-js");"#),
        [Reach::Unbounded(Unbounded::NamespaceType)]
    );
}

const RUNTIME: &[&str] = &["solid-js", "@solidjs/signals", "@solidjs/web"];

fn solid_use_of(
    files: &[(&str, &str)],
    resolved: Option<&AttestedImportIndex>,
) -> Option<SolidUse> {
    let facts = files
        .iter()
        .map(|(path, source)| {
            (
                *path,
                solid_facts::ast::extract(*path, source).expect("the source parses"),
            )
        })
        .collect::<Vec<_>>();
    solid_use(
        RUNTIME,
        facts.iter().map(|(path, ast)| (*path, ast)),
        resolved,
    )
    .map(|(_, _, kind)| kind)
}

#[test]
fn a_project_that_names_no_runtime_module_and_writes_no_jsx_uses_none() {
    assert_eq!(
        solid_use_of(
            &[
                (
                    "/p/index.ts",
                    r#"import { helper } from "./helper";
                           import { readFileSync } from "node:fs";
                           const load = (name: string) => import(name);
                           export const value = helper(readFileSync);"#
                ),
                ("/p/helper.ts", "export const helper = (x: unknown) => x;"),
            ],
            None
        ),
        None
    );
}

#[test]
fn every_module_reference_into_the_runtime_is_a_use() {
    for source in [
        r#"import { createSignal } from "solid-js";"#,
        r#"import type { Component } from "solid-js";"#,
        r#"import "@solidjs/web";"#,
        r#"import { jsx } from "@solidjs/web/jsx-runtime";"#,
        r#"import { createStore } from "@solidjs/signals";"#,
        r#"export * from "solid-js";"#,
        r#"export { untrack } from "solid-js";"#,
        r#"const solid = await import("solid-js");"#,
        r#"const solid = require("solid-js");"#,
        r#"import S = require("solid-js");"#,
        r#"type A = import("solid-js").Accessor<number>;"#,
    ] {
        assert_eq!(
            solid_use_of(&[("/p/index.ts", source)], None),
            Some(SolidUse::RuntimeModule),
            "{source}"
        );
    }
}

#[test]
fn any_jsx_is_a_use_whatever_its_runtime() {
    assert_eq!(
        solid_use_of(&[("/p/App.tsx", "export const view = <div />;")], None),
        Some(SolidUse::Jsx)
    );
    assert_eq!(
        solid_use_of(&[("/p/App.tsx", "export const view = <></>;")], None),
        Some(SolidUse::Jsx)
    );
}

#[test]
fn a_package_reference_the_facts_cannot_place_fails_closed() {
    // No attested resolution: which package `tailwindcss/plugin` is, and
    // what it depends on, is unknown.
    for source in [
        r#"import plugin from "tailwindcss/plugin";"#,
        r#"import type { StyleRule } from "@vanilla-extract/css";"#,
        r#"import T = require("some-package");"#,
        r#"import solid from "solid-jsx";"#,
        r#"import direct from "../node_modules/solid-js/dist/solid.js";"#,
    ] {
        assert_eq!(
            solid_use_of(&[("/p/index.ts", source)], None),
            Some(SolidUse::Unknown),
            "{source}"
        );
    }
    // A dialect that names no runtime package rules nothing out.
    assert!(solid_use(&[], std::iter::empty(), None).is_some());
}

fn row(text: &str, span: Span, resolved: &Path, resolution: ImportResolution) -> AttestedImport {
    AttestedImport {
        span,
        text: text.into(),
        resolution,
        resolved_path: resolved.display().to_string().into(),
        included_path: "".into(),
        symlink_path: "".into(),
        extension: ".d.ts".into(),
        package_name: None,
        package_version: None,
        package_manifest: None,
        resolver_package_name: None,
        resolver_package_version: None,
    }
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

#[test]
fn a_package_reference_is_judged_by_its_installed_dependency_closure() {
    let root = std::env::temp_dir().join(format!(
        "solid-checker-release-scope-closure-{}",
        std::process::id()
    ));
    let modules = root.join("node_modules");
    // `free` -> `leaf` (and an optional peer and optional dependency that
    // are not installed): no runtime anywhere.
    write(
        &modules.join("free/package.json"),
        r#"{"name":"free","dependencies":{"leaf":"1"},"optionalDependencies":{"fsevents":"2"},
               "peerDependencies":{"maybe":"1"},"peerDependenciesMeta":{"maybe":{"optional":true}}}"#,
    );
    write(&modules.join("free/dist/index.d.ts"), "export {};");
    // An unnamed nested manifest beside the ESM output is passed over.
    write(
        &modules.join("free/dist/package.json"),
        r#"{"type":"module"}"#,
    );
    write(&modules.join("leaf/package.json"), r#"{"name":"leaf"}"#);
    // `wraps` -> `bridge` (installed in `wraps`'s own node_modules), which
    // peer-depends on the runtime.
    write(
        &modules.join("wraps/package.json"),
        r#"{"name":"wraps","dependencies":{"bridge":"1"}}"#,
    );
    write(&modules.join("wraps/index.d.ts"), "export {};");
    write(
        &modules.join("wraps/node_modules/bridge/package.json"),
        r#"{"name":"bridge","peerDependencies":{"solid-js":"^2"}}"#,
    );
    // `broken` names a required dependency that is not installed.
    write(
        &modules.join("broken/package.json"),
        r#"{"name":"broken","dependencies":{"missing":"1"}}"#,
    );
    write(&modules.join("broken/index.d.ts"), "export {};");
    let root = std::fs::canonicalize(&root).unwrap();
    let modules = root.join("node_modules");

    let case = |text: &str, resolved: &Path, resolution: ImportResolution| {
        let source = format!("import x from \"{text}\";\nexport default x;\n");
        let ast = solid_facts::ast::extract("/p/index.ts", &source).unwrap();
        let literal = source.find('"').unwrap() as u32;
        let span = Span::new(literal, literal + text.len() as u32 + 2);
        let mut index = AttestedImportIndex::default();
        index.insert_file("/p/index.ts", vec![row(text, span, resolved, resolution)]);
        solid_use(RUNTIME, [("/p/index.ts", &ast)], Some(&index)).map(|(_, _, kind)| kind)
    };
    assert_eq!(
        case(
            "free",
            &modules.join("free/dist/index.d.ts"),
            ImportResolution::NodeModules
        ),
        None
    );
    assert_eq!(
        case(
            "wraps",
            &modules.join("wraps/index.d.ts"),
            ImportResolution::NodeModules
        ),
        Some(SolidUse::RuntimeDependency)
    );
    assert_eq!(
        case(
            "broken",
            &modules.join("broken/index.d.ts"),
            ImportResolution::NodeModules
        ),
        Some(SolidUse::Unknown)
    );
    assert_eq!(
        case("absent", Path::new(""), ImportResolution::Unresolved),
        Some(SolidUse::Unknown)
    );
    // A relative specifier that lands in a file the analysis does not
    // read asks nothing of it; one that walks into a package is that
    // package.
    assert_eq!(
        case(
            "./styles.css",
            &root.join("styles.css"),
            ImportResolution::Relative
        ),
        None
    );
    assert_eq!(
        case(
            "./node_modules/wraps/index.js",
            &modules.join("wraps/index.d.ts"),
            ImportResolution::Relative
        ),
        Some(SolidUse::RuntimeDependency)
    );
    // A `paths` alias that the compiler resolved into a project file is
    // the project's own code, which the census reads directly.
    let ast = solid_facts::ast::extract("/p/index.ts", "import x from \"@app/x\";").unwrap();
    let other = solid_facts::ast::extract("/p/x.ts", "export default 1;").unwrap();
    let mut index = AttestedImportIndex::default();
    index.insert_file(
        "/p/index.ts",
        vec![row(
            "@app/x",
            Span::new(14, 22),
            Path::new("/p/x.ts"),
            ImportResolution::NonRelative,
        )],
    );
    assert_eq!(
        solid_use(
            RUNTIME,
            [("/p/index.ts", &ast), ("/p/x.ts", &other)],
            Some(&index)
        ),
        None
    );
    // An alias the compiler resolved into a runtime package is a use.
    let mut into_runtime = row(
        "@app/x",
        Span::new(14, 22),
        &modules.join("solid-js/types/index.d.ts"),
        ImportResolution::NonRelative,
    );
    into_runtime.package_name = Some("solid-js".into());
    let mut index = AttestedImportIndex::default();
    index.insert_file("/p/index.ts", vec![into_runtime]);
    assert_eq!(
        solid_use(RUNTIME, [("/p/index.ts", &ast)], Some(&index)).map(|(_, _, kind)| kind),
        Some(SolidUse::RuntimeModule)
    );
    std::fs::remove_dir_all(&root).unwrap();
}
