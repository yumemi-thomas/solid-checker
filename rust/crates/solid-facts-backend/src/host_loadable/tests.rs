use super::*;

#[test]
fn stylesheet_executable_closure_uses_style_conditions_and_tracks_nested_inputs() {
    let scratch = std::env::temp_dir().join(format!("host-css-closure-{}", std::process::id()));
    let app = scratch.join("app");
    let package = app.join("node_modules/css-provider");
    fs::create_dir_all(&package).unwrap();
    let app = fs::canonicalize(&app).unwrap();
    let package = fs::canonicalize(&package).unwrap();
    let config =
        crate::host_config::parse(Path::new("vite.config.ts"), "export default {plugins:[]}")
            .unwrap();
    let main = app.join("main.css");
    fs::write(&main, "@import 'css-provider';").unwrap();
    fs::write(package.join("package.json"), r#"{"exports":{".":{"style":{"production":"./production.css","default":"./development.css"},"import":"./runtime.js"}}}"#).unwrap();
    fs::write(package.join("runtime.js"), "throw 0").unwrap();
    fs::write(
        package.join("development.css"),
        "@reference '../../outside.css';",
    )
    .unwrap();
    fs::write(package.join("production.css"), ".x{}").unwrap();
    let outside = app.join("outside.css");
    fs::write(&outside, ".x{}").unwrap();
    let mut inputs = Vec::new();
    assert!(stylesheet_closure(
        std::slice::from_ref(&main),
        &config,
        &mut inputs
    ));
    for path in [
        &main,
        &outside,
        &package.join("package.json"),
        &package.join("production.css"),
        &package.join("development.css"),
    ] {
        assert!(
            inputs
                .iter()
                .any(|input| fs::canonicalize(input).ok().as_ref() == Some(path)),
            "{}",
            path.display()
        );
    }
    for directive in [
        "@plugin './plugin.js';",
        "@\\70lugin './plugin.js';",
        "@plu\\\ngin './plugin.js';",
        "@con/**/fig './config.js';",
    ] {
        fs::write(&outside, directive).unwrap();
        assert!(
            !stylesheet_closure(std::slice::from_ref(&main), &config, &mut Vec::new()),
            "{directive}"
        );
    }
    // A definitively absent literal is a transform failure under the
    // successful-build premise; absence remains an input, not a guessed file.
    fs::remove_file(&outside).unwrap();
    let mut inputs = Vec::new();
    assert!(stylesheet_closure(
        std::slice::from_ref(&main),
        &config,
        &mut inputs
    ));
    assert!(inputs.contains(&package.join("../../outside.css")));
    fs::write(&outside, "@plugin './plugin.js';").unwrap();
    assert!(!stylesheet_closure(
        std::slice::from_ref(&main),
        &config,
        &mut Vec::new()
    ));
    // Vite implicitly activates import even for a style resolver. Earlier
    // author-order import must not be skipped in favor of clean style.
    fs::write(
        package.join("package.json"),
        r#"{"exports":{".":{"import":"./evil.css","style":"./production.css"}}}"#,
    )
    .unwrap();
    fs::write(package.join("evil.css"), "@plugin './plugin.js';").unwrap();
    assert!(!stylesheet_closure(
        std::slice::from_ref(&main),
        &config,
        &mut Vec::new()
    ));
    // Whitespace/comments in composition cannot hide an imported plugin.
    fs::write(
        &main,
        ".x { composes : x from\t/*comment*/ './outside.css'; }",
    )
    .unwrap();
    assert!(!stylesheet_closure(
        std::slice::from_ref(&main),
        &config,
        &mut Vec::new()
    ));
    fs::write(&main, "@import 'css-provider';").unwrap();
    assert_eq!(
        normalized_css("@import './e\\76il.css';"),
        "@import './e\\76il.css';"
    );
    fs::write(&main, "@import './e\\76il.css';").unwrap();
    fs::write(app.join("evil.css"), ".x{}").unwrap();
    assert!(!stylesheet_closure(
        std::slice::from_ref(&main),
        &config,
        &mut Vec::new()
    ));
    fs::write(&main, "@import 'css-provider';").unwrap();
    // Declaration/JS exports cannot answer for unknown CSS style resolution.
    fs::write(
        package.join("package.json"),
        r#"{"exports":{".":{"import":"./runtime.js"}}}"#,
    )
    .unwrap();
    assert!(!stylesheet_closure(
        std::slice::from_ref(&main),
        &config,
        &mut Vec::new()
    ));
    fs::remove_dir_all(scratch).unwrap();
}

fn exported(exports: &Target, key: &str, production: bool) -> Option<String> {
    super::exported(exports, key, production, false, false)
}

fn attested(root: &Path, name: &str, version: &str) -> AttestedImport {
    AttestedImport {
        span: solid_facts::core::Span::new(0, 1),
        text: name.into(),
        resolution: ImportResolution::NodeModules,
        resolved_path: root.join("index.d.ts").to_str().unwrap().into(),
        included_path: "".into(),
        symlink_path: "".into(),
        extension: ".d.ts".into(),
        package_name: Some(name.into()),
        package_version: Some(version.into()),
        package_manifest: Some(root.join("package.json").to_str().unwrap().into()),
        resolver_package_name: Some(name.into()),
        resolver_package_version: Some(version.into()),
    }
}

#[test]
fn importer_directory_observations_replay_absence_and_refresh_next_pass() {
    let app = std::env::temp_dir().join(format!("host-package-lookups-{}", std::process::id()));
    fs::create_dir_all(app.join("src")).unwrap();
    let package = app.join("node_modules/reactive-package");
    fs::create_dir_all(&package).unwrap();
    let manifest =
        r#"{"name":"reactive-package","version":"1.0.0","type":"module","exports":"./index.js"}"#;
    fs::write(package.join("package.json"), manifest).unwrap();
    fs::write(package.join("index.js"), "export const x = 1;").unwrap();
    let app = fs::canonicalize(app).unwrap();
    let mut cache = PackageLoads::default();
    let mut first = Vec::new();
    let selected = cache
        .candidates(&app.join("src/a.ts"), "reactive-package", &mut first)
        .unwrap();
    let shadow = app.join("src/node_modules/reactive-package");
    assert!(first.contains(&shadow.join("package.json")));
    let mut second = Vec::new();
    assert_eq!(
        cache.candidates(&app.join("src/b.ts"), "reactive-package", &mut second),
        Some(selected)
    );
    assert_eq!(first, second);
    assert_eq!(cache.candidates.len(), 1);
    fs::create_dir_all(&shadow).unwrap();
    fs::write(shadow.join("package.json"), manifest).unwrap();
    fs::write(shadow.join("index.js"), "export const x = 2;").unwrap();
    let refreshed = PackageLoads::default()
        .candidates(&app.join("src/a.ts"), "reactive-package", &mut Vec::new())
        .unwrap();
    assert_eq!(refreshed.0, fs::canonicalize(&shadow).unwrap());
    for root in [&shadow, &app.join("node_modules/reactive-package")] {
        fs::remove_file(root.join("index.js")).unwrap();
        fs::remove_file(root.join("package.json")).unwrap();
        fs::remove_dir(root).unwrap();
    }
    fs::remove_dir(app.join("src/node_modules")).unwrap();
    fs::remove_dir(app.join("src")).unwrap();
    fs::remove_dir(app.join("node_modules")).unwrap();
    fs::remove_dir(app).unwrap();
}

#[test]
fn vite_resource_files_and_refusal_twins() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let app = std::env::temp_dir().join(format!(
        "host-resources-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(app.join("node_modules/styles/dist")).unwrap();
    fs::create_dir_all(app.join("node_modules/tailwindcss")).unwrap();
    fs::write(
        app.join("node_modules/tailwindcss/package.json"),
        r#"{"exports":{".":{"style":"./index.css","import":"./runtime.js"}}}"#,
    )
    .unwrap();
    fs::write(app.join("node_modules/tailwindcss/index.css"), ".x{}").unwrap();
    let app = fs::canonicalize(app).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir/inferred-host-spa/resource-cases.json");
    let cases: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture).unwrap()).unwrap();
    for case in cases {
        fs::write(
            app.join("node_modules/styles/package.json"),
            case["manifest"].as_str().unwrap(),
        )
        .unwrap();
        fs::write(
            app.join("node_modules/styles/dist/style.css"),
            case["css"].as_str().unwrap(),
        )
        .unwrap();
        fs::write(app.join("node_modules/styles/dist/runtime.js"), "throw 0").unwrap();
        fs::write(app.join("local.css"), case["css"].as_str().unwrap()).unwrap();
        fs::write(app.join("local.pcss"), case["css"].as_str().unwrap()).unwrap();
        fs::write(app.join("local.module.css"), case["css"].as_str().unwrap()).unwrap();
        fs::write(app.join("logo.svg"), "<svg/>").unwrap();
        fs::write(app.join("style.scss"), ".x{color:red}").unwrap();
        fs::write(app.join("payload.txt"), "throw 0").unwrap();
        let mut config = crate::host_config::parse(
            Path::new("vite.config.ts"),
            case["config"]
                .as_str()
                .unwrap_or("export default {plugins:[]}"),
        )
        .unwrap();
        if let Some(plugin) = case["plugin"].as_str() {
            config.plugins.insert(plugin.into());
        }
        let mut inputs = Vec::new();
        let result = resource(
            &app,
            &app.join("main.ts"),
            case["request"].as_str().unwrap(),
            &config,
            &mut inputs,
        );
        assert_eq!(
            result.is_some(),
            case["loadable"].as_bool().unwrap(),
            "{}",
            case["name"]
        );
        if let Some(result) = result {
            assert!(!result.entries.is_empty());
            assert!(result.entries.iter().all(|path| inputs.contains(path)));
        }
    }
    let config =
        crate::host_config::parse(Path::new("vite.config.ts"), "export default {plugins:[]}")
            .unwrap();
    let mut inputs = Vec::new();
    assert!(
        resource(
            &app,
            &app.join("main.ts"),
            "./absent.svg",
            &config,
            &mut inputs
        )
        .is_none()
    );
    assert!(
        inputs.contains(&app.join("./absent.svg")),
        "absence is a cache input"
    );
    assert!(css_without_executable_inputs("@\\69mport 'hidden.css';"));
    assert!(!css_without_executable_inputs("@con/**/fig 'hidden.ts';"));
    assert!(!css_without_executable_inputs("@\\70lugin 'hidden.ts';"));
    assert!(css_without_executable_inputs(
        "@import \"tailwindcss\"; .x{color:red}"
    ));
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn resource_resolution_is_exact_per_occurrence_and_never_overrides_a_requested_unknown() {
    use solid_facts::runtime_resolution::{RuntimeOutcome, RuntimeResolutionIndex};
    let resource = Resource {
        entries: vec![PathBuf::from("/app/logo.svg")],
    };
    let span = solid_facts::core::Span::new(7, 17);
    let mut runtime = RuntimeResolutionIndex::default();
    assert!(!resource.matches_outcome(runtime.outcome("/app/main.ts", span, "./logo.svg")));
    runtime.insert(
        "/app/main.ts",
        span,
        "./logo.svg",
        RuntimeOutcome::File {
            path: "/app/logo.svg".into(),
            physical_path: "/app/logo.svg".into(),
        },
    );
    assert!(resource.matches_outcome(runtime.outcome("/app/main.ts", span, "./logo.svg")));
    assert!(!resource.matches_outcome(runtime.outcome("/app/main.ts", span, "./logo.svg?url")));
    for outcome in [
        RuntimeOutcome::Unknown,
        RuntimeOutcome::External,
        RuntimeOutcome::Builtin,
        RuntimeOutcome::File {
            path: "/app/other.svg".into(),
            physical_path: "/app/other.svg".into(),
        },
        RuntimeOutcome::File {
            path: "/app/logo.svg".into(),
            physical_path: "/store/logo.svg".into(),
        },
    ] {
        assert!(!resource.matches_outcome(&outcome));
    }
}

#[test]
fn attested_install_core_triple_commonjs_and_store_twins() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let app = std::env::temp_dir().join(format!(
        "host-installed-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(&app).unwrap();
    let app = fs::canonicalize(app).unwrap();
    let importer = app.join("main.ts");
    let dialect = crate::dialect::default_dialect().vocabulary;
    for &(name, version) in dialect.audited_installation() {
        let root = app.join("node_modules").join(name);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("index.d.ts"),
            "export declare const value: number;",
        )
        .unwrap();
        fs::write(root.join("client.js"), "export const value = 1;").unwrap();
        let manifest = |version: &str| {
            serde_json::to_vec(&serde_json::json!({"name":name,"version":version,"type":"module","exports":{".":{"browser":"./client.js","default":"./missing.js"}}})).unwrap()
        };
        fs::write(root.join("package.json"), manifest(version)).unwrap();
        let mut row = attested(&root, name, version);
        let load = |row: &AttestedImport| {
            loadable(
                &importer,
                name,
                row,
                dialect,
                &mut Vec::new(),
                &Default::default(),
            )
        };
        assert!(matches!(
            load(&row),
            Some(PackageLoadability::AuditedDialect(_))
        ));
        row.resolver_package_version = Some("wrong".into());
        assert!(load(&row).is_none());
        fs::write(root.join("package.json"), manifest("2.0.0-rc.999")).unwrap();
        let row = attested(&root, name, "2.0.0-rc.999");
        assert!(matches!(load(&row), Some(PackageLoadability::Installed(_))));
        fs::remove_file(root.join("client.js")).unwrap();
        assert!(load(&row).is_none());
    }
    let name = "commonjs-package";
    let root = app.join("node_modules").join(name);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{"name":"commonjs-package","version":"1.0.0","main":"./index.cjs"}"#,
    )
    .unwrap();
    fs::write(root.join("index.cjs"), "module.exports={value:1};").unwrap();
    fs::write(
        root.join("index.d.ts"),
        "export declare const value: number;",
    )
    .unwrap();
    let row = attested(&root, name, "1.0.0");
    assert!(
        loadable(
            &importer,
            name,
            &row,
            dialect,
            &mut Vec::new(),
            &Default::default()
        )
        .is_some()
    );
    assert!(
        loadable(
            &importer,
            name,
            &row,
            dialect,
            &mut Vec::new(),
            &std::collections::BTreeSet::from([name.into()])
        )
        .is_none()
    );
    #[cfg(unix)]
    {
        let store = app.join("node_modules/.pnpm/client-package@1.0.0/node_modules/client-package");
        fs::create_dir_all(&store).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/inferred-host-reachability");
        fs::copy(
            fixture.join("package-client.json"),
            store.join("package.json"),
        )
        .unwrap();
        fs::write(store.join("client.js"), "export const value=1;").unwrap();
        fs::write(
            store.join("index.d.ts"),
            "export declare const value:number;",
        )
        .unwrap();
        std::os::unix::fs::symlink(&store, app.join("node_modules/client-package")).unwrap();
        let mut row = attested(&store, "client-package", "1.0.0");
        row.symlink_path = app
            .join("node_modules/client-package/index.d.ts")
            .to_str()
            .unwrap()
            .into();
        let load = || {
            loadable(
                &importer,
                "client-package",
                &row,
                dialect,
                &mut Vec::new(),
                &Default::default(),
            )
        };
        assert!(load().is_some());
        for name in ["package-hidden.json", "package-browser-false.json"] {
            fs::copy(fixture.join(name), store.join("package.json")).unwrap();
            assert!(load().is_none(), "{name}");
        }
        fs::copy(
            fixture.join("package-client.json"),
            store.join("package.json"),
        )
        .unwrap();
        let mut inputs = Vec::new();
        assert!(
            loadable(
                &importer,
                "client-package",
                &row,
                dialect,
                &mut inputs,
                &Default::default()
            )
            .is_some()
        );
        assert!(inputs.contains(&store.join("client.js")));
        assert!(inputs.contains(&store.join("package.json")));
        assert!(inputs.contains(&app.join("node_modules/client-package")));
        // A shallower install shadows the store. Exact declaration root
        // disagreement must refuse instead of name/version matching it.
        fs::create_dir_all(app.join("src/node_modules/client-package")).unwrap();
        fs::copy(
            fixture.join("package-client.json"),
            app.join("src/node_modules/client-package/package.json"),
        )
        .unwrap();
        fs::write(
            app.join("src/node_modules/client-package/client.js"),
            "export const value=2;",
        )
        .unwrap();
        assert!(
            loadable(
                &app.join("src/main.ts"),
                "client-package",
                &row,
                dialect,
                &mut Vec::new(),
                &Default::default()
            )
            .is_none()
        );
        // Bun's store has the same canonical-root premise; retargeting an
        // install changes both declaration identity and cached input paths.
        let bun = app.join("node_modules/.bun/client-package@1.0.0/node_modules/client-package");
        fs::create_dir_all(bun.parent().unwrap()).unwrap();
        fs::remove_file(app.join("node_modules/client-package")).unwrap();
        fs::rename(&store, &bun).unwrap();
        std::os::unix::fs::symlink(&bun, app.join("node_modules/client-package")).unwrap();
        assert!(
            loadable(
                &importer,
                "client-package",
                &row,
                dialect,
                &mut Vec::new(),
                &Default::default()
            )
            .is_none()
        );
        let bun_row = attested(&bun, "client-package", "1.0.0");
        assert!(
            loadable(
                &importer,
                "client-package",
                &bun_row,
                dialect,
                &mut Vec::new(),
                &Default::default()
            )
            .is_some()
        );
    }
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn client_export_order_and_closed_subpaths() {
    let parse = |json: &str| serde_json::from_str::<Target>(json).unwrap();
    let exports = parse(
        r#"{".":{"node":"./server.js","browser":{"development":"./dev.js","default":"./client.js"},"default":"./fallback.js"},"./hidden":null,"./features/*":"./features/*.js"}"#,
    );
    assert_eq!(exported(&exports, ".", false).as_deref(), Some("./dev.js"));
    assert_eq!(
        exported(&exports, ".", true).as_deref(),
        Some("./client.js")
    );
    assert_eq!(exported(&exports, "./hidden", false), None);
    assert_eq!(exported(&exports, "./missing", false), None);
    assert_eq!(
        exported(&exports, "./features/a", false).as_deref(),
        Some("./features/a.js")
    );
    assert_eq!(
        exported(
            &parse(r#"{"./*":"./broad/*.js","./features/*":"./specific/*.js"}"#),
            "./features/a",
            false
        )
        .as_deref(),
        Some("./specific/a.js")
    );
    assert_eq!(
        exported(
            &parse(r#"{"default":"./first.js","browser":"./second.js"}"#),
            ".",
            false
        )
        .as_deref(),
        Some("./first.js")
    );
    assert_eq!(
        exported(
            &parse(r#"{"import":null,"default":"./fallback.js"}"#),
            ".",
            false
        ),
        None
    );
    assert_eq!(
        exported(
            &parse(r#"{"browser":{"unknown":"./no.js"},"import":"./yes.js"}"#),
            ".",
            false
        )
        .as_deref(),
        Some("./yes.js")
    );
    assert_eq!(exported(&parse(r#"["./a.js","./b.js"]"#), ".", false), None);
    assert!(serde_json::from_str::<Ordered>(r#"{"exports":"./a.js","exports":"./b.js"}"#).is_err());
}

#[test]
fn canonical_package_loader_is_checked_for_every_selected_entry() {
    let scratch = std::env::temp_dir().join(format!("host-package-loader-{}", std::process::id()));
    let root = scratch.join("node_modules/styles");
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    fs::write(root.join("index.d.ts"), "export {};").unwrap();
    fs::write(root.join("index.js"), "export {};").unwrap();
    for ext in [
        "css", "pcss", "postcss", "less", "sass", "scss", "styl", "stylus", "sss", "svg", "json",
        "unknown",
    ] {
        fs::write(
            root.join(format!("style.{ext}")),
            "@import './missing.css'; .x{",
        )
        .unwrap();
        for (text, exports) in [
            ("styles", format!(r#"{{".":"./style.{ext}"}}"#)),
            ("styles/theme", format!(r#"{{"./theme":"./style.{ext}"}}"#)),
            (
                "styles",
                format!(r#"{{".":{{"production":"./style.{ext}","default":"./index.js"}}}}"#),
            ),
            (
                "styles",
                format!(r#"{{".":{{"solid":"./style.{ext}","default":"./index.js"}}}}"#),
            ),
        ] {
            fs::write(
                root.join("package.json"),
                format!(
                    r#"{{"name":"styles","version":"1.0.0","type":"module","exports":{exports}}}"#
                ),
            )
            .unwrap();
            let mut inputs = Vec::new();
            assert_eq!(
                loadable(
                    &scratch.join("main.ts"),
                    text,
                    &attested(&root, "styles", "1.0.0"),
                    crate::dialect::default_dialect().vocabulary,
                    &mut inputs,
                    &Default::default()
                )
                .is_some(),
                ext != "unknown",
                "{text}: {exports}"
            );
            assert!(
                inputs.contains(&root.join(format!("style.{ext}"))),
                "withheld input still recorded"
            );
        }
    }
    fs::write(root.join("package.json"), r#"{"name":"styles","version":"1.0.0","type":"module","exports":{".":"./index.js","./theme":"./index.js"}}"#).unwrap();
    for text in ["styles", "styles/theme"] {
        assert!(
            loadable(
                &scratch.join("main.ts"),
                text,
                &attested(&root, "styles", "1.0.0"),
                crate::dialect::default_dialect().vocabulary,
                &mut Vec::new(),
                &Default::default()
            )
            .is_some()
        );
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn manifest_entry_regular_file_and_refusal_twins() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "host-loadable-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    fs::write(root.join("client.js"), "export const value = 1;").unwrap();
    fs::write(root.join("index.cjs"), "module.exports = {value:1};").unwrap();
    let fixture = |manifest: &str, key: &str| {
        entry(
            &root,
            manifest.as_bytes(),
            key,
            false,
            false,
            &mut Vec::new(),
        )
    };
    assert_eq!(
        fixture(
            r#"{"exports":{".":{"browser":"./client.js","default":"./missing.js"}}}"#,
            "."
        ),
        Some(root.join("client.js"))
    );
    assert_eq!(
        fixture(r#"{"main":"index.cjs"}"#, "."),
        Some(root.join("index.cjs"))
    );
    assert_eq!(
        fixture(r#"{"module":"./client.js","main":"./missing.js"}"#, "."),
        Some(root.join("client.js"))
    );
    for manifest in [
        r#"{"exports":{".":"./missing.js"},"main":"client.js"}"#,
        r#"{"exports":{"./other":"./client.js"},"main":"client.js"}"#,
        r#"{"exports":"./client.js","browser":false}"#,
        r#"{"exports":"./client.js","browser":{"./client.js":false}}"#,
        r#"{"exports":"../outside.js"}"#,
        r#"{"exports":"./index.d.ts"}"#,
    ] {
        assert_eq!(fixture(manifest, "."), None, "{manifest}");
    }
    assert_eq!(
        fixture(r#"{"exports":{"./public":"./client.js"}}"#, "./public"),
        Some(root.join("client.js"))
    );
    assert_eq!(
        fixture(r#"{"exports":{"./public":"./client.js"}}"#, "./hidden"),
        None
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("client.js"), root.join("internal.js")).unwrap();
        assert_eq!(
            fixture(r#"{"exports":"./internal.js"}"#, "."),
            Some(root.join("client.js"))
        );
        std::os::unix::fs::symlink(std::env::temp_dir(), root.join("escape")).unwrap();
        assert_eq!(fixture(r#"{"exports":"./escape/other.js"}"#, "."), None);
    }
    fs::remove_dir_all(root).unwrap();
}
