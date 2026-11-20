#[test]
fn extends_chain_follows_relative_and_package_configs_exactly() {
    let root = std::env::temp_dir().join(format!(
        "sc-extends-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let write = |path: &str, text: &str| {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    write(
        "app/tsconfig.json",
        r#"{ "extends": ["./base", "@cfg/strict"] }"#,
    );
    write(
        "app/base.json",
        "{ // comment\n \"extends\": \"shared/tsconfig.json\" }",
    );
    write("app/node_modules/shared/tsconfig.json", "{}");
    write(
        "node_modules/@cfg/strict/package.json",
        r#"{ "tsconfig": "strict.json" }"#,
    );
    write("node_modules/@cfg/strict/strict.json", "{}");
    let chain = super::tsconfig_extends_chain(&root.join("app/tsconfig.json")).unwrap();
    let names = chain
        .iter()
        .map(|path| {
            path.strip_prefix(root.canonicalize().unwrap())
                .unwrap()
                .display()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "app/base.json",
            "app/node_modules/shared/tsconfig.json",
            "node_modules/@cfg/strict/strict.json"
        ]
    );
    // An `exports` map is TypeScript's resolution: refused.
    write(
        "node_modules/@cfg/strict/package.json",
        r#"{ "exports": { "./*": "./*" } }"#,
    );
    assert!(super::tsconfig_extends_chain(&root.join("app/tsconfig.json")).is_none());
    // A cycle, and a missing file, refuse.
    write("cycle/a.json", r#"{ "extends": "./b.json" }"#);
    write("cycle/b.json", r#"{ "extends": "./a.json" }"#);
    assert!(super::tsconfig_extends_chain(&root.join("cycle/a.json")).is_none());
    write("missing/tsconfig.json", r#"{ "extends": "./gone" }"#);
    assert!(super::tsconfig_extends_chain(&root.join("missing/tsconfig.json")).is_none());
    // No `extends`: an empty chain.
    write("plain/tsconfig.json", "{}");
    assert_eq!(
        super::tsconfig_extends_chain(&root.join("plain/tsconfig.json")),
        Some(vec![])
    );
    std::fs::remove_dir_all(&root).unwrap();
}

use super::*;

#[test]
fn tsconfig_resolver_activity_is_conservative() {
    let path = Path::new("vite.config.ts");
    for (source, active) in [
        ("export default {plugins:[]}", false),
        (
            "export default {plugins:[],resolve:{tsconfigPaths:false}}",
            false,
        ),
        (
            "export default {plugins:[],resolve:{tsconfigPaths:true}}",
            true,
        ),
        (
            "export default {plugins:[],resolve:{tsconfigPaths:undefined}}",
            true,
        ),
        (
            "import {defineConfig} from 'vite'; export default defineConfig(({mode}) => ({plugins:[],resolve:{tsconfigPaths:mode}}))",
            true,
        ),
        (
            "import {defineConfig} from 'vite'; export default defineConfig(({mode}) => mode ? {plugins:[]} : {plugins:[],resolve:{tsconfigPaths:true}})",
            true,
        ),
        (
            "import paths from 'vite-tsconfig-paths'; export default {plugins:[paths()]}",
            true,
        ),
        (
            "import paths from 'vite-tsconfig-paths'; export default {plugins:[]}",
            true,
        ),
    ] {
        assert_eq!(
            parse(path, source).unwrap().tsconfig_paths,
            active,
            "{source}"
        );
    }
    for source in [
        "export default unknown",
        "export default {plugins:[],resolve:unknown}",
        "export default {plugins:[],resolve:{tsconfigPaths:unknown}}",
        "import paths from 'vite-tsconfig-paths'; export default {plugins:[paths({root:'..'})]}",
        "import paths from 'vite-tsconfig-paths'; export default {plugins:[paths({projects:['./tsconfig.app.json']})]}",
        "import paths from 'vite-tsconfig-paths'; export default {plugins:[paths(options)]}",
    ] {
        assert!(parse(path, source).is_none(), "{source}");
    }
}

#[test]
fn package_resolver_controls_and_branch_exclusions_remain_closed() {
    let path = Path::new("vite.config.ts");
    let default = parse(
        path,
        "export default {plugins:[],resolve:{dedupe:[]},optimizeDeps:{exclude:[]}}",
    )
    .unwrap();
    assert!(!default.package_dedupe);
    assert!(default.optimize_exclude.is_empty());
    let changed = parse(path, "export default {plugins:[],resolve:{dedupe:['solid-js']},optimizeDeps:{exclude:['commonjs-package']}}").unwrap();
    assert!(changed.package_dedupe);
    assert!(changed.optimize_exclude.contains("commonjs-package"));
    let branch = parse(path, "import {defineConfig} from 'vite';export default defineConfig(({mode}) => mode === 'a' ? {plugins:[],optimizeDeps:{exclude:['a']}} : {plugins:[],optimizeDeps:{exclude:['b']}})").unwrap();
    assert_eq!(
        branch.optimize_exclude,
        BTreeSet::from(["a".into(), "b".into()])
    );
    for source in [
        "export default {plugins:[],resolve:{conditions:['node']}}",
        "export default {plugins:[],resolve:{mainFields:['main']}}",
        "export default {plugins:[],optimizeDeps:{disabled:true}}",
        "export default {plugins:[],resolve:{dedupe:true}}",
    ] {
        assert!(parse(path, source).is_none(), "{source}");
    }
}

#[test]
fn admission_fixture_pairs_are_a_closed_config_grammar() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir/inferred-host-spa/admission-cases.json");
    let cases: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    for case in cases.as_array().unwrap() {
        assert_eq!(
            parse(
                Path::new("vite.config.ts"),
                case["source"].as_str().unwrap()
            )
            .is_some(),
            case["accepted"].as_bool().unwrap(),
            "{}",
            case["name"]
        );
    }
    assert_eq!(
        jsonc("{ /* */ \"paths\": {\"~/*\":[\"src/*\",],}, // eof\n}"),
        Some(serde_json::json!({"paths":{"~/*":["src/*"]}}))
    );
    assert!(jsonc("{\"x\": import('server')}").is_none());
    assert!(jsonc("{/*unterminated").is_none());
}

#[test]
fn allowlisted_calls_are_literal_in_every_branch() {
    let prefix = "import {defineConfig} from 'vite'; import solid from '@solidjs/vite-plugin'; import {fileRoutes} from 'filesystem-routing/vite';";
    let source = format!(
        "{prefix} export default defineConfig(({{mode}}) => mode === 'production' ? {{plugins:[solid({{start:true}}),fileRoutes()]}} : {{plugins:[solid({{start:true}}),fileRoutes()]}});"
    );
    assert_eq!(
        parse(Path::new("vite.config.ts"), &source).unwrap().mode,
        EntryMode::ClientStart
    );
    for tail in [
        "({plugins:[solid({start:true}), ...extra]})",
        "({plugins:[solid({start:true,ssr:true})]})",
        "mode === 'production' ? {plugins:[solid({start:true})]} : {plugins:[unknown()]}",
        "({plugins:[solid({start:true}), {configureServer(s){s.ssrLoadModule('./src/App.tsx')}}]})",
        "({plugins:[solid({start:true})], build:{ssr:'src/App.tsx'}})",
    ] {
        assert!(
            parse(
                Path::new("vite.config.ts"),
                &format!("{prefix} export default defineConfig(({{mode}}) => {tail});")
            )
            .is_none(),
            "{tail}"
        );
    }
}

#[test]
fn optional_plugins_require_closed_nonexecuting_options() {
    for (module, exported, good, bad) in [
        ("@unocss/vite", "default", "{configFile:false}", "{}"),
        (
            "vite-plugin-solid-svg",
            "default",
            "{svgo:{enabled:false}}",
            "{}",
        ),
        (
            "vite-plugin-pwa",
            "VitePWA",
            "{strategies:'generateSW'}",
            "{strategies:'injectManifest'}",
        ),
        (
            "@tanstack/router-plugin/vite",
            "tanstackRouter",
            "{target:'solid'}",
            "{target:'solid',plugins:[]}",
        ),
        (
            "filesystem-routing/vite",
            "fileRoutes",
            "{types:true}",
            "{buildInputs:['ssr']}",
        ),
        ("vite-tsconfig-paths", "default", "{}", "{parseNative:true}"),
    ] {
        let import = if exported == "default" {
            "import p".to_owned()
        } else {
            format!("import {{{exported} as p}}")
        };
        for (options, accepted) in [(good, true), (bad, false)] {
            let source =
                format!("{import} from '{module}'; export default {{plugins:[p({options})]}};");
            assert_eq!(
                parse(Path::new("vite.config.ts"), &source).is_some(),
                accepted,
                "{source}"
            );
        }
    }
}
