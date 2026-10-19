use super::*;

#[test]
fn graph_observations_do_not_replace_boundary_validation() {
    let root = std::env::temp_dir().join(format!("host-graph-observation-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("input.ts");
    fs::write(&path, "before").unwrap();
    let mut manifest = BrowserRootManifest::default();
    let before = graph_identity(&mut manifest, &path).unwrap();
    fs::write(&path, "after!").unwrap();
    assert_eq!(graph_identity(&mut manifest, &path), Some(before.clone()));
    let index = ProjectHostIndex::build(manifest, &[], &[]);
    assert!(validate_inputs(&index).is_err());
    let mut replay = BrowserRootManifest::default();
    assert_ne!(graph_identity(&mut replay, &path), Some(before));
    fs::remove_file(path).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn parallel_input_fingerprints_keep_order_absence_and_content_changes() {
    let root = std::env::temp_dir().join(format!("host-fingerprints-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let files = (0..160)
        .map(|n| root.join(format!("{n}.ts")))
        .collect::<Vec<_>>();
    for (n, path) in files.iter().enumerate().take(159) {
        fs::write(path, format!("{n:03}")).unwrap();
    }
    let paths = files.iter().map(PathBuf::as_path).collect::<Vec<_>>();
    let before = identities(&paths);
    assert_eq!(before[159].as_deref(), Some("absent"));
    assert!(before[..159].iter().all(Option::is_some));
    assert!(before[..159].windows(2).all(|pair| pair[0] != pair[1]));
    fs::write(&files[31], "new").unwrap();
    fs::write(&files[159], "159").unwrap();
    let after = identities(&paths);
    let changed = before
        .iter()
        .zip(&after)
        .enumerate()
        .filter_map(|(n, (a, b))| (a != b).then_some(n))
        .collect::<Vec<_>>();
    assert_eq!(changed, vec![31, 159]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn independent_html_scripts_preserve_external_entries_only() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir/inferred-host-reachability");
    for name in ["classic-independent.html", "inline-module-independent.html"] {
        assert_eq!(
            html_entries(&fs::read_to_string(fixture.join(name)).unwrap()),
            Some(vec!["src/independent.ts".into()]),
            "{name}"
        );
    }
    for name in [
        "classic-interference.html",
        "inline-module-interference.html",
    ] {
        assert!(
            html_entries(&fs::read_to_string(fixture.join(name)).unwrap()).is_none(),
            "{name}"
        );
    }
    assert_eq!(
        html_entries("<script type='module'>throw 'error';</script>"),
        None
    );
    for source in [
        "location='/elsewhere'",
        "window.stop()",
        "document.querySelector('script').remove()",
        "const d=document; d['open']()",
        "unknown()",
    ] {
        assert!(
            html_entries(&format!(
                "<script>{source}</script><script type='module' src='/src/main.ts'></script>"
            ))
            .is_none(),
            "{source}"
        );
    }
    assert!(
        html_entries(
            "<script src='/external.js'></script><script type='module' src='/src/main.ts'></script>"
        )
        .is_none()
    );
}

#[test]
fn exact_audited_lazy_route_id_requires_no_eager_picks() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir/inferred-host-reachability");
    let config = |options: &str| {
        host_config::parse(Path::new("vite.config.ts"), &format!("import {{fileRoutes}} from 'filesystem-routing/vite';export default {{plugins:[fileRoutes({options})]}};")).unwrap()
    };
    let lazy = config("{dir:'route-lazy',types:true}");
    assert!(lazy_route_manifest(&directory, &lazy, &mut Vec::new()));
    assert!(!lazy_route_manifest(
        &directory,
        &config("{dir:'route-eager',types:true}"),
        &mut Vec::new()
    ));
    assert!(!lazy_route_manifest(
        &directory,
        &config("{dir:'route-lazy',codeSplitting:false}"),
        &mut Vec::new()
    ));
    assert!(!lazy_route_manifest(
        &directory,
        &config("{dir:'missing-routes'}"),
        &mut Vec::new()
    ));
    let mut absent = lazy.clone();
    absent.plugins.clear();
    assert!(!lazy_route_manifest(&directory, &absent, &mut Vec::new()));
    assert!(host_config::parse(Path::new("vite.config.ts"), "import {fileRoutes} from 'filesystem-routing/vite';export default {plugins:[fileRoutes({moduleId:'virtual:other'})]}").is_none());
}

#[test]
fn default_extension_and_index_probes_require_the_first_congruent_real_file() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let directory = std::env::temp_dir().join(format!(
        "host-extensions-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(&directory).unwrap();
    let directory = fs::canonicalize(directory).unwrap();
    let importer = directory.join("main.ts");
    let mut config =
        host_config::parse(Path::new("vite.config.ts"), "export default {plugins:[]}").unwrap();
    let agrees = |config: &Config, text: &str, target: &Path| {
        exact_local_target(
            config,
            importer.to_str().unwrap(),
            text,
            target.to_str().unwrap(),
        )
    };
    fs::write(directory.join("App.tsx"), "export default 1").unwrap();
    assert!(agrees(&config, "./App", &directory.join("App.tsx")));
    assert!(!agrees(&config, "./App", &directory.join("App.ts")));
    fs::write(directory.join("App.ts"), "export default 2").unwrap();
    assert!(agrees(&config, "./App", &directory.join("App.ts")));
    assert!(!agrees(&config, "./App", &directory.join("App.tsx")));
    fs::remove_file(directory.join("App.ts")).unwrap();
    fs::write(directory.join("App.js"), "export default 3").unwrap();
    assert!(agrees(&config, "./App", &directory.join("App.js")));
    assert!(!agrees(&config, "./App", &directory.join("App.tsx")));
    assert!(!agrees(&config, "./App.js", &directory.join("App.tsx")));
    fs::remove_file(directory.join("App.js")).unwrap();
    assert!(!agrees(&config, "./App?raw", &directory.join("App.tsx")));
    fs::create_dir_all(directory.join("folder")).unwrap();
    fs::write(directory.join("folder/index.ts"), "export default 1").unwrap();
    assert!(agrees(
        &config,
        "./folder",
        &directory.join("folder/index.ts")
    ));
    fs::write(directory.join("folder/index.js"), "export default 2").unwrap();
    assert!(agrees(
        &config,
        "./folder",
        &directory.join("folder/index.js")
    ));
    assert!(!agrees(
        &config,
        "./folder",
        &directory.join("folder/index.ts")
    ));
    fs::remove_file(directory.join("folder/index.js")).unwrap();
    fs::write(
        directory.join("folder/package.json"),
        "{\"main\":\"different.js\"}",
    )
    .unwrap();
    assert!(!agrees(
        &config,
        "./folder",
        &directory.join("folder/index.ts")
    ));
    fs::remove_file(directory.join("folder/package.json")).unwrap();
    fs::write(directory.join("folder.ts"), "export default 3").unwrap();
    assert!(agrees(&config, "./folder", &directory.join("folder.ts")));
    assert!(!agrees(
        &config,
        "./folder",
        &directory.join("folder/index.ts")
    ));
    fs::write(directory.join("App"), "runtime bytes").unwrap();
    assert!(!agrees(&config, "./App", &directory.join("App.tsx")));
    fs::remove_file(directory.join("App")).unwrap();
    config
        .aliases
        .insert("~".into(), Some(directory.to_str().unwrap().into()));
    assert!(agrees(&config, "~/App", &directory.join("App.tsx")));
    config.default_extensions = false;
    assert!(!agrees(&config, "./App", &directory.join("App.tsx")));
    assert!(agrees(&config, "./App.tsx", &directory.join("App.tsx")));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(directory.join("App.tsx"), directory.join("link.tsx")).unwrap();
        config.default_extensions = true;
        assert!(!agrees(&config, "./link", &directory.join("link.tsx")));
    }
    for source in [
        "export default {plugins:[],resolve:{tsconfigPaths:true}}",
        "import paths from 'vite-tsconfig-paths'; export default {plugins:[paths()]}",
        "export default mode ? {plugins:[]} : {plugins:[],resolve:{tsconfigPaths:true}}",
    ] {
        // A config factory's mode is admitted as inert config data.
        let source = if source.contains("mode ?") {
            "import {defineConfig} from 'vite'; export default defineConfig(({mode}) => mode ? {plugins:[]} : {plugins:[],resolve:{tsconfigPaths:true}})"
        } else {
            source
        };
        let parsed = host_config::parse(Path::new("vite.config.ts"), source).unwrap();
        assert!(!parsed.default_extensions);
        assert!(parsed.tsconfig_paths);
    }
    assert!(
        host_config::parse(
            Path::new("vite.config.ts"),
            "export default {plugins:[],resolve:{extensions:['.tsx']}}"
        )
        .is_none()
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn tsrx_probes_preserve_selected_root_instead_of_falling_through() {
    assert_eq!(
        client_root(|path| ["src/entry-client.tsrx", "src/App.tsx"].contains(&path)),
        Some(("src/entry-client.tsrx".into(), false))
    );
    assert_eq!(
        client_root(|path| ["src/App.tsrx", "src/app.tsx"].contains(&path)),
        Some(("src/App.tsrx".into(), true))
    );
}

#[test]
fn previous_parameter_execution_must_complete_before_a_later_default_or_body() {
    for (source, complete) in [
        ("function f(x=throws(),y=later()){} f();", false),
        ("function f(x=throws(),y=later()){} f(false);", true),
        ("function f(x=throws(),y=later()){} f(void 0);", false),
        ("function f(x=true,y=later()){} f();", true),
        ("function f({x},y=later()){} f({x:true});", false),
    ] {
        let ast = solid_facts::ast::extract("inputs.ts", source).unwrap();
        let call = ast.calls.iter().max_by_key(|call| call.span.start).unwrap();
        assert_eq!(
            parameter_initialization_complete(&ast, &ast.functions[0], call, 0),
            complete,
            "{source}"
        );
    }
}

#[test]
fn default_inputs_require_known_argument_position_and_runtime_undefined() {
    for (source, index, expected) in [
        ("f()", 0, true),
        ("f(false)", 0, false),
        ("f(void 0)", 0, true),
        ("f(undefined)", 0, false),
        ("f(...xs)", 0, false),
        ("f(false,...xs)", 0, false),
        ("f(void 0,...xs)", 0, false),
        ("f(void 0,throwing())", 0, false),
        ("f(...xs,void 0)", 1, false),
        ("f(false)", 1, true),
        ("f((void 0) as boolean | undefined)", 0, true),
    ] {
        let ast = solid_facts::ast::extract("inputs.ts", source).unwrap();
        assert_eq!(
            omitted_or_undefined(&ast.calls[0], index, &ast.host_undefined_arguments),
            expected,
            "{source}"
        );
    }
}

#[test]
fn tsconfig_resolver_withholds_every_default_bare_or_aliased_selection() {
    let mut config = host_config::parse(
        Path::new("vite.config.ts"),
        "export default {plugins:[],resolve:{tsconfigPaths:true}}",
    )
    .unwrap();
    config.aliases.insert("/alias".into(), Some("/src".into()));
    config.aliases.insert("./alias".into(), Some("/src".into()));
    for (request, required) in [
        ("@solidjs/web", true),
        ("~/App.ts", true),
        (".package", true),
        ("/alias/App.ts", true),
        ("./alias/App.ts", true),
        ("./App.ts", false),
        ("../App.ts", false),
        ("/src/App.ts", false),
    ] {
        assert_eq!(
            resolver_selection_required(&config, request),
            required,
            "{request}"
        );
    }
    config.tsconfig_paths = false;
    assert!(!resolver_selection_required(&config, "@solidjs/web"));
    assert!(!resolver_selection_required(&config, "./alias/App.ts"));
}

#[test]
fn tsconfig_resolver_selection_requires_exact_runtime_agreement() {
    use solid_facts::runtime_resolution::{RuntimeOutcome, RuntimeResolutionIndex};
    let Ok(producer) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let scratch =
        std::env::temp_dir().join(format!("host-tsconfig-selection-{}", std::process::id()));
    let directory = scratch.join("app");
    fs::create_dir_all(directory.join("src")).unwrap();
    fs::create_dir_all(scratch.join("shadow-base/@solidjs")).unwrap();
    fs::write(
        scratch.join("shadow-base/@solidjs/web.js"),
        "export const isServer = true;",
    )
    .unwrap();
    let directory = fs::canonicalize(directory).unwrap();
    fs::write(directory.join("package.json"), "{\"private\":true}").unwrap();
    fs::write(
        directory.join("index.html"),
        "<script type='module' src='/src/main.ts'></script>",
    )
    .unwrap();
    let source = "import {isServer} from '@solidjs/web'; new Date(); if (!isServer) selected(); function selected() {}";
    let main = directory.join("src/main.ts");
    fs::write(&main, source).unwrap();
    fs::write(
        directory.join("src/shadow.js"),
        "export const isServer = true;",
    )
    .unwrap();
    let project = directory.join("tsconfig.app.json");
    fs::write(&project, r#"{"compilerOptions":{"strict":true,"module":"ESNext","moduleResolution":"Bundler","target":"ES2022"},"include":["src/*.ts"]}"#).unwrap();
    let package = directory.join("node_modules/@solidjs/web");
    fs::create_dir_all(package.join("dist")).unwrap();
    fs::write(package.join("package.json"), r#"{"name":"@solidjs/web","version":"2.0.0-rc.13","type":"module","exports":{".":{"types":"./index.d.ts","browser":{"development":"./dist/web.dev.js","observe":"./dist/web.observe.js","default":"./dist/web.js"},"node":{"development":"./dist/server.dev.js","observe":"./dist/server.observe.js","default":"./dist/server.js"},"default":"./dist/web.js"}}}"#).unwrap();
    // Exact published signature, not a relaxed callback or literal typing.
    fs::write(
        package.join("index.d.ts"),
        "export declare const isServer: boolean;",
    )
    .unwrap();
    let dialect = crate::dialect::default_dialect();
    for target in dialect.vocabulary.host_boolean_exports()[0].runtime_targets {
        fs::write(
            package.join(target),
            format!("export const isServer = {};", target.contains("server")),
        )
        .unwrap();
    }
    let typescript =
        crate::TypeFactsSession::open(&producer, project.to_str().unwrap(), &[]).unwrap();
    let (mut session, _) = crate::NativeIncrementalSession::open_pipelined(
        dialect,
        project.to_str().unwrap().into(),
        typescript,
    )
    .unwrap();
    let facts = session.analyze().unwrap();
    let file = facts
        .files
        .iter()
        .find(|file| file.path.as_str() == main.to_str().unwrap())
        .unwrap();
    let import = &file.ast.imports[0];
    let cases: serde_json::Value =
        serde_json::from_slice(
            &fs::read(repository.join(
                "fixtures/reactive-ir/inferred-host-reachability/resolver-selection-cases.json",
            ))
            .unwrap(),
        )
        .unwrap();
    for config_source in [
        "export default {plugins:[],resolve:{tsconfigPaths:true}}",
        "import {defineConfig} from 'vite'; export default defineConfig(({mode}) => ({plugins:[],resolve:{tsconfigPaths:mode}}))",
        "import {defineConfig} from 'vite'; export default defineConfig(({mode}) => mode ? {plugins:[]} : {plugins:[],resolve:{tsconfigPaths:true}})",
        "import paths from 'vite-tsconfig-paths'; export default {plugins:[paths()]}",
    ] {
        let config = host_config::parse(&directory.join("vite.config.ts"), config_source).unwrap();
        assert!(config.tsconfig_paths);
        for case in cases.as_array().unwrap() {
            fs::write(
                directory.join("tsconfig.json"),
                serde_json::to_vec(&case["tsconfig"]).unwrap(),
            )
            .unwrap();
            fs::write(
                directory.join("inherited.json"),
                r#"{"compilerOptions":{"paths":{"@solidjs/web":["./src/shadow.js"]}}}"#,
            )
            .unwrap();
            let config = if let Some(source) = case["config"].as_str() {
                let parsed = host_config::parse(&directory.join("vite.config.ts"), source);
                if case["configRefused"] == true {
                    assert!(parsed.is_none(), "{case}");
                    continue;
                }
                parsed.unwrap()
            } else if case["resolver"] == false {
                host_config::parse(
                    &directory.join("vite.config.ts"),
                    "export default {plugins:[]}",
                )
                .unwrap()
            } else {
                config.clone()
            };
            let prepared = DiscoveryInputs {
                directory: directory.clone(),
                config,
                paths: BTreeMap::new(),
                config_inputs: Vec::new(),
            };
            let mut observed = facts.as_ref().clone();
            if case["runtime"] != "absent" {
                let outcome = match case["runtime"].as_str().unwrap() {
                    "client" => Some(package.join("dist/web.js")),
                    "server" => Some(package.join("dist/server.js")),
                    "shadow" => Some(directory.join("src/shadow.js")),
                    "token-shadow" => Some(
                        directory
                            .parent()
                            .unwrap()
                            .join("shadow-base/@solidjs/web.js"),
                    ),
                    _ => None,
                }
                .map_or(RuntimeOutcome::Unknown, |path| RuntimeOutcome::File {
                    path: path.to_str().unwrap().into(),
                    physical_path: path.to_str().unwrap().into(),
                });
                let mut runtime = RuntimeResolutionIndex::default();
                runtime.insert(
                    main.to_str().unwrap(),
                    import.span,
                    import.module.as_str(),
                    outcome,
                );
                observed.runtime_resolutions = Some(runtime);
            }
            let constants = execution_constants(
                &prepared,
                &observed,
                file,
                dialect.vocabulary,
                &mut BrowserRootManifest::default(),
            );
            let expected = case["browser"].as_bool().unwrap();
            assert_eq!(!constants.is_empty(), expected, "{config_source}: {case}");
            let index =
                discovered_index_with_inputs(&prepared, &observed, dialect.vocabulary, true)
                    .unwrap();
            let start = u64::try_from(source.find("selected()").unwrap()).unwrap();
            assert_eq!(
                index.browser_proof_site(main.to_str().unwrap(), start, start + 10),
                expected,
                "{config_source}: {case}"
            );
        }
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn native_execution_fixture_sites_keep_function_boundaries_and_edge_refusals() {
    let Ok(producer) = std::env::var("SOLID_TYPEFACTS_BIN") else {
        return;
    };
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for name in [
        "reachability",
        "alias-congruent",
        "alias-divergent",
        "certification-clean",
    ] {
        let directory =
            fs::canonicalize(repository.join(format!("fixtures/reactive-ir/inferred-host-{name}")))
                .unwrap();
        let project = directory.join("tsconfig.json").to_str().unwrap().to_owned();
        let typescript = crate::TypeFactsSession::open(&producer, &project, &[]).unwrap();
        let dialect = crate::dialect::default_dialect();
        let (mut session, _) =
            crate::NativeIncrementalSession::open_pipelined(dialect, project, typescript).unwrap();
        let facts = session.analyze().unwrap();
        if name == "alias-congruent" {
            let entry = facts
                .files
                .iter()
                .find(|file| file.path.as_str() == directory.join("src/main.tsx").to_str().unwrap())
                .unwrap();
            let import = entry
                .ast
                .imports
                .iter()
                .find(|import| import.module == "~")
                .unwrap();
            assert!(
                runtime_import(&facts, entry, import),
                "a proven value use installs the congruent alias execution edge"
            );
        }
        let provisional = discovered_index(&directory, &facts, dialect.vocabulary, false).unwrap();
        let prepared = discovery_inputs(&directory, &facts, dialect.vocabulary).unwrap();
        let package_checked =
            discovered_index_with_inputs(&prepared, &facts, dialect.vocabulary, true).unwrap();
        assert!(
            package_checked.browser_scope_count() <= provisional.browser_scope_count(),
            "package checks only withhold scopes: {name}"
        );
        if provisional.is_empty() {
            assert!(
                package_checked.is_empty(),
                "an empty provisional graph cannot gain authority"
            );
        }
        let index = provisional.with_callback_invocations(
            solid_reactive_ir::callback_host::browser_callback_invocations(
                &facts,
                dialect.vocabulary,
            ),
        );
        let cases: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join("host-cases.json")).unwrap()).unwrap();
        let mut mismatches = Vec::new();
        for case in cases.as_array().unwrap() {
            let path = directory.join(case["path"].as_str().unwrap());
            let source = fs::read_to_string(&path).unwrap();
            let text = case["text"].as_str().unwrap();
            let start = u64::try_from(source.find(text).unwrap()).unwrap();
            let actual = index.browser_site(
                path.to_str().unwrap(),
                start,
                start + u64::try_from(text.len()).unwrap(),
            );
            if actual != case["browser"].as_bool().unwrap() {
                mismatches.push(format!("{name}: {case}; browser actual={actual}"));
            }
            if let Some(proof) = case.get("proof").and_then(serde_json::Value::as_bool) {
                let actual = index.browser_proof_site(
                    path.to_str().unwrap(),
                    start,
                    start + u64::try_from(text.len()).unwrap(),
                );
                if actual != proof {
                    mismatches.push(format!("{name}: {case}; proof actual={actual}"));
                }
            }
        }
        assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
        if name == "reachability" || name == "alias-divergent" {
            use solid_facts::runtime_resolution::{RuntimeOutcome, RuntimeResolutionIndex};
            let importer = directory.join(if name == "reachability" {
                "src/runtime-entry.ts"
            } else {
                "src/main.tsx"
            });
            let source = facts
                .files
                .iter()
                .find(|file| file.path.as_str() == importer.to_str().unwrap())
                .unwrap();
            let import = source
                .ast
                .imports
                .iter()
                .find(|import| {
                    import.module
                        == if name == "reachability" {
                            "./runtime-helper"
                        } else {
                            "~"
                        }
                })
                .unwrap();
            let target = directory.join(if name == "reachability" {
                "src/runtime-helper.ts"
            } else {
                "src/helper.ts"
            });
            for (outcome, expected) in [
                (RuntimeOutcome::Unknown, false),
                (
                    RuntimeOutcome::File {
                        path: target.to_str().unwrap().into(),
                        physical_path: directory.join("src/server.ts").to_str().unwrap().into(),
                    },
                    false,
                ),
                (
                    RuntimeOutcome::File {
                        path: target.to_str().unwrap().into(),
                        physical_path: target.to_str().unwrap().into(),
                    },
                    true,
                ),
            ] {
                let mut observed = facts.as_ref().clone();
                let mut runtime = RuntimeResolutionIndex::default();
                if name == "reachability" {
                    let main = facts
                        .files
                        .iter()
                        .find(|file| {
                            file.path.as_str() == directory.join("src/main.tsx").to_str().unwrap()
                        })
                        .unwrap();
                    let entry = main
                        .ast
                        .imports
                        .iter()
                        .find(|import| import.module == "./runtime-entry.ts")
                        .unwrap();
                    runtime.insert(
                        main.path.shared(),
                        entry.span,
                        entry.module.as_str(),
                        RuntimeOutcome::File {
                            path: importer.to_str().unwrap().into(),
                            physical_path: importer.to_str().unwrap().into(),
                        },
                    );
                }
                runtime.insert(
                    importer.to_str().unwrap(),
                    import.span,
                    import.module.as_str(),
                    outcome,
                );
                observed.runtime_resolutions = Some(runtime);
                let index =
                    discovered_index(&directory, &observed, dialect.vocabulary, false).unwrap();
                assert_eq!(
                    index.browser_site(target.to_str().unwrap(), 0, 1),
                    expected && name != "reachability",
                    "{name}"
                );
                assert!(index.manifest.runtime_resolution.is_some());
            }
        }
    }
}

#[test]
fn literal_alias_congruence_requires_the_same_single_target() {
    let mut config = host_config::parse(
        Path::new("vite.config.ts"),
        "export default {plugins:[],resolve:{alias:{'~':'/app/src'}}};",
    )
    .unwrap();
    let paths = BTreeMap::from([("~/*".into(), vec!["/app/src/*".into()])]);
    assert!(congruent_alias(&config, &paths, "~/helper"));
    assert!(congruent_alias(&config, &paths, "./helper"));
    assert!(congruent_alias(&config, &BTreeMap::new(), "~/helper"));
    config
        .aliases
        .insert("~".into(), Some("/app/server".into()));
    assert!(!congruent_alias(&config, &paths, "~/helper"));
    config.aliases.insert("~".into(), None);
    assert!(!congruent_alias(&config, &paths, "~/helper"));
    config.plugins.insert("vite-tsconfig-paths".into());
    assert!(!congruent_alias(&config, &paths, "~/helper"));
    config.aliases.insert("~".into(), Some("/app/src".into()));
    assert!(congruent_alias(&config, &paths, "~/helper"));
}

#[test]
fn aggregate_import_uses_cannot_cross_a_server_function_boundary() {
    for (source, client) in [
        (
            "import { value } from './helper'; console.log(value);",
            true,
        ),
        (
            "import { value } from './helper'; type T = typeof value; function server() { 'use server'; return value; }",
            false,
        ),
        (
            "import { value } from './helper'; console.log(value); function server() { 'use server'; return value; }",
            false,
        ),
        (
            "import { value } from './helper'; console.log(value); function server(value: number) { 'use server'; return value; }",
            true,
        ),
    ] {
        let ast = solid_facts::ast::extract("imports.ts", source).unwrap();
        let binding = &ast.imports[0].bindings[0];
        assert!(binding.runtime_referenced);
        assert_eq!(
            import_references_are_client_only(&ast, binding.local.span),
            client,
            "{source}"
        );
    }
    let ast = solid_facts::ast::extract(
        "imports.ts",
        "import { value } from './helper'; type T = typeof value;",
    )
    .unwrap();
    assert!(!ast.imports[0].bindings[0].runtime_referenced);
}

#[test]
fn conventional_invocation_fixture_twins_name_the_refusal_site() {
    let scratch = std::env::temp_dir().join(format!(
        "host-conventional-selection-{}",
        std::process::id()
    ));
    fs::create_dir_all(scratch.join("app")).unwrap();
    let scratch = fs::canonicalize(scratch).unwrap();
    fs::create_dir_all(scratch.join("shadow-base")).unwrap();
    let app = fs::canonicalize(scratch.join("app")).unwrap();
    let project = app.join("tsconfig.json");
    fs::write(&project, "{}").unwrap();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let cases: serde_json::Value = serde_json::from_slice(
        &fs::read(repository.join(
            "fixtures/reactive-ir/inferred-host-reachability/conventional-selection-cases.json",
        ))
        .unwrap(),
    )
    .unwrap();
    for case in cases.as_array().unwrap() {
        if case.get("browserMap").is_some() || case.get("workspaceBrowser").is_some() {
            continue;
        }
        if !cfg!(unix)
            && ["manifestSymlink", "workspaceSymlink", "configSymlink"]
                .iter()
                .any(|key| case[*key] == true)
        {
            continue;
        }
        for path in [
            app.join("package.json"),
            scratch.join("package.json"),
            app.join("vite.config.ts"),
        ] {
            if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_symlink()) {
                fs::remove_file(path).unwrap();
            }
        }
        if let Some(config) = case["outsideConfig"].as_str() {
            fs::write(scratch.join("other.ts"), config).unwrap();
        }
        let parent_config = scratch.join("vite.config.ts");
        if let Some(config) = case["parentConfig"].as_str() {
            fs::write(&parent_config, config).unwrap();
        } else if parent_config.exists() {
            fs::remove_file(&parent_config).unwrap();
        }
        let mut package = serde_json::json!({"private":true,"name":"inferred-host-spa"});
        if let Some(scripts) = case.get("scripts") {
            package["scripts"] = scripts.clone();
        }
        fs::write(
            app.join("package.json"),
            serde_json::to_vec(&package).unwrap(),
        )
        .unwrap();
        let mut workspace = serde_json::json!({"private":true});
        if let Some(scripts) = case.get("workspaceScripts") {
            workspace["scripts"] = scripts.clone();
        }
        fs::write(
            scratch.join("package.json"),
            serde_json::to_vec(&workspace).unwrap(),
        )
        .unwrap();
        fs::write(
            app.join("vite.config.ts"),
            case["config"]
                .as_str()
                .unwrap_or("export default {plugins:[]};"),
        )
        .unwrap();
        if let Some(name) = case["configFile"].as_str() {
            fs::write(app.join(name), "export default {};").unwrap();
        }
        #[cfg(unix)]
        for (key, path, target) in [
            (
                "manifestSymlink",
                app.join("package.json"),
                scratch.join("app-manifest.json"),
            ),
            (
                "workspaceSymlink",
                scratch.join("package.json"),
                scratch.join("workspace-manifest.json"),
            ),
            (
                "configSymlink",
                app.join("vite.config.ts"),
                scratch.join("linked-config.ts"),
            ),
        ] {
            if case[key] == true {
                fs::rename(&path, &target).unwrap();
                std::os::unix::fs::symlink(target, path).unwrap();
            }
        }
        let mut inputs = Vec::new();
        let decision = application_inputs(&app, &project, &mut inputs)
            .and_then(|()| configuration(&app, &mut inputs));
        assert_eq!(
            decision.is_ok(),
            case["browser"] == true,
            "{case}: {decision:?}"
        );
        if let Some(note) = case["note"].as_str() {
            let refusal = decision.unwrap_err();
            assert!(refusal.message().contains(note), "{case}: {refusal:?}");
            assert!(inputs.contains(&refusal.path));
            if case.get("workspaceScripts").is_some() {
                assert_eq!(refusal.path, scratch.join("package.json"));
            }
        }
        if let Some(config) = case["mutatedParentConfig"].as_str() {
            let app_manifest = inferred_host_input_digest(&app.join("package.json"));
            let parent_manifest = inferred_host_input_digest(&scratch.join("package.json"));
            let before = inferred_host_input_digest(&parent_config);
            fs::write(&parent_config, config).unwrap();
            assert_ne!(before, inferred_host_input_digest(&parent_config));
            assert_eq!(
                app_manifest,
                inferred_host_input_digest(&app.join("package.json"))
            );
            assert_eq!(
                parent_manifest,
                inferred_host_input_digest(&scratch.join("package.json"))
            );
            let refusal = application_inputs(&app, &project, &mut Vec::new()).unwrap_err();
            assert_eq!(refusal.path, scratch.join("package.json"));
            assert!(refusal.message().contains(case["note"].as_str().unwrap()));
        }
        if case.get("outsideConfig").is_some() {
            // A refused invocation cannot regain authority when the
            // unselected outside config changes into an innocuous one.
            fs::write(scratch.join("other.ts"), "export default {plugins:[]};").unwrap();
            assert!(
                application_inputs(&app, &project, &mut Vec::new())
                    .and_then(|()| configuration(&app, &mut Vec::new()))
                    .is_err(),
                "{case}"
            );
        }
        #[cfg(unix)]
        for (key, path, target) in [
            (
                "manifestSymlink",
                app.join("package.json"),
                scratch.join("app-manifest.json"),
            ),
            (
                "workspaceSymlink",
                scratch.join("package.json"),
                scratch.join("workspace-manifest.json"),
            ),
            (
                "configSymlink",
                app.join("vite.config.ts"),
                scratch.join("linked-config.ts"),
            ),
        ] {
            if case[key] == true {
                let pointer = inferred_host_input_digest(&path).unwrap();
                let bytes = inferred_host_input_digest(&target).unwrap();
                fs::write(&target, "different mutable target bytes").unwrap();
                assert_eq!(pointer, inferred_host_input_digest(&path).unwrap());
                assert_ne!(bytes, inferred_host_input_digest(&target).unwrap());
                assert!(
                    application_inputs(&app, &project, &mut Vec::new())
                        .and_then(|()| configuration(&app, &mut Vec::new()))
                        .is_err()
                );
            }
        }
        if let Some(name) = case["configFile"].as_str() {
            fs::remove_file(app.join(name)).unwrap();
        }
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn conventional_selection_and_browser_map_inputs_invalidate_authority() {
    let scratch = std::env::temp_dir().join(format!("host-selection-cache-{}", std::process::id()));
    let app = scratch.join("app");
    fs::create_dir_all(app.join("src")).unwrap();
    let scratch = fs::canonicalize(scratch).unwrap();
    let app = fs::canonicalize(app).unwrap();
    let project = app.join("tsconfig.json");
    let package = app.join("package.json");
    fs::write(&project, "{}").unwrap();
    fs::write(
        &package,
        r#"{"private":true,"scripts":{"build":"vite build"}}"#,
    )
    .unwrap();
    fs::write(scratch.join("package.json"), r#"{"private":true}"#).unwrap();
    fs::write(app.join("vite.config.ts"), "export default {plugins:[]};").unwrap();
    let source = app.join("src/main.ts");
    fs::write(&source, "export const selected = true;").unwrap();
    let inputs = inferred_host_input_paths_for_project(&app, &project);
    assert!(inputs.contains(&app));
    assert!(inputs.contains(&package));
    assert!(inputs.contains(&scratch.join("package.json")));
    assert!(inputs.contains(&app.join("src/package.json")));
    let before = inferred_host_input_digest(&app).unwrap();
    let variant = app.join("vite.config.production.ts");
    fs::write(&variant, "export default {plugins:[]};").unwrap();
    assert_ne!(before, inferred_host_input_digest(&app).unwrap());
    assert!(inferred_host_input_paths_for_project(&app, &project).contains(&variant));
    fs::remove_file(&variant).unwrap();
    let before = inferred_host_input_digest(&package).unwrap();
    fs::write(
        &package,
        r#"{"private":true,"scripts":{"build":"vite build --mode production"}}"#,
    )
    .unwrap();
    assert_ne!(before, inferred_host_input_digest(&package).unwrap());
    assert!(application(&app, &project).is_err());
    for path in [
        &package,
        &scratch.join("package.json"),
        &app.join("src/package.json"),
    ] {
        let before = inferred_host_input_digest(path).unwrap();
        fs::write(path, r#"{"private":true,"browser":{"@solidjs/web":false}}"#).unwrap();
        assert_ne!(before, inferred_host_input_digest(path).unwrap());
        let mut manifest = BrowserRootManifest::default();
        assert!(browser_selection_required(&source, &mut manifest));
        assert!(manifest.inputs.contains_key(path.to_str().unwrap()));
        fs::write(path, r#"{"private":true,"browser":{}}"#).unwrap();
        assert!(!browser_selection_required(
            &source,
            &mut BrowserRootManifest::default()
        ));
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn tsconfig_resolver_activity_inputs_invalidate_cached_authority() {
    let scratch = std::env::temp_dir().join(format!(
        "host-resolver-activity-inputs-{}",
        std::process::id()
    ));
    fs::create_dir_all(scratch.join("src")).unwrap();
    let app = fs::canonicalize(&scratch).unwrap();
    let project = app.join("tsconfig.app.json");
    fs::write(app.join("package.json"), "{\"private\":true}").unwrap();
    fs::write(&project, "{}").unwrap();
    let vite = app.join("vite.config.ts");
    fs::write(&vite, "export default {plugins:[]}").unwrap();
    let inputs = inferred_host_input_paths_for_project(&app, &project);
    assert!(inputs.contains(&vite));
    let before = inferred_host_input_digest(&vite).unwrap();
    fs::write(
        &vite,
        "export default {plugins:[],resolve:{tsconfigPaths:true}}",
    )
    .unwrap();
    assert_ne!(before, inferred_host_input_digest(&vite).unwrap());
    fs::write(
        &vite,
        "import paths from 'vite-tsconfig-paths'; export default {plugins:[paths()]}",
    )
    .unwrap();
    let inputs = inferred_host_input_paths_for_project(&app, &project);
    let plugin = app.join("node_modules/vite-tsconfig-paths/package.json");
    assert!(inputs.contains(&vite));
    assert!(inputs.contains(&plugin));
    let absent = inferred_host_input_digest(&plugin).unwrap();
    fs::create_dir_all(plugin.parent().unwrap()).unwrap();
    fs::write(
        &plugin,
        "{\"name\":\"vite-tsconfig-paths\",\"version\":\"unknown\"}",
    )
    .unwrap();
    assert_ne!(absent, inferred_host_input_digest(&plugin).unwrap());
    assert!(inferred_host_input_paths_for_project(&app, &project).contains(&plugin));
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn analyzed_config_name_and_inheritance_are_exact_discovery_inputs() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "host-config-name-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let app = scratch.join("app");
    fs::create_dir_all(&app).unwrap();
    let app = fs::canonicalize(app).unwrap();
    fs::write(app.join("package.json"), "{\"private\":true}").unwrap();
    let base = app.parent().unwrap().join("tsconfig.base.json");
    fs::write(
        &base,
        "{\"compilerOptions\":{\"paths\":{\"~/*\":[\"./app/src/*\"]}}}",
    )
    .unwrap();
    let project = app.join("tsconfig.app.json");
    fs::write(&project, "{\"extends\":\"../tsconfig.base.json\"}").unwrap();
    assert!(application(&app, &project).is_ok());
    let inputs = inferred_host_input_paths_for_project(&app, &project);
    assert!(inputs.contains(&project));
    assert!(inputs.contains(&base));
    assert_eq!(
        paths_targets(&app, &project).unwrap()["~/*"],
        vec![app.join("src/*").to_str().unwrap().to_owned()]
    );
    fs::write(&base, "{\"references\":[]}").unwrap();
    assert!(application(&app, &project).is_ok());
    fs::write(&base, "{\"references\":[{\"path\":\"../external\"}]}").unwrap();
    let refusal = application(&app, &project).unwrap_err();
    assert_eq!(refusal.path, base);
    assert!(refusal.reason.contains("references"));
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn refused_configuration_inputs_do_not_scale_with_application_inventory() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "host-refusal-inputs-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ));
    fs::create_dir_all(scratch.join("src")).unwrap();
    let app = fs::canonicalize(&scratch).unwrap();
    fs::write(app.join("package.json"), "{\"private\":true}").unwrap();
    let project = app.join("tsconfig.json");
    fs::write(&project, "{}").unwrap();
    let config = app.join("vite.config.ts");
    fs::write(
        &config,
        "import unknown from 'unknown-plugin'; export default unknown();",
    )
    .unwrap();
    let before = inferred_host_input_paths_for_project(&app, &project);
    for index in 0..256 {
        fs::write(
            app.join(format!("src/mod{index}.tsx")),
            "not valid TypeScript {{{",
        )
        .unwrap();
    }
    assert_eq!(
        before,
        inferred_host_input_paths_for_project(&app, &project)
    );
    // One package plus 20 PostCSS candidates per ancestor, six Vite
    // candidates, one tsconfig and the fixture premise marker.
    assert!(before.len() <= app.ancestors().count() * 21 + 9);
    assert!(CONFIGS.iter().all(|name| before.contains(&app.join(name))));
    assert!(before.contains(&project));
    assert!(!before.contains(&app.join("src")));
    fs::write(&config, "export default {plugins: []};").unwrap();
    let after = inferred_host_input_paths_for_project(&app, &project);
    assert!(after.contains(&app.join("src/mod255.tsx")));
    // A competing config must invalidate the successful grammar branch.
    fs::write(app.join("vite.config.js"), "export default {};").unwrap();
    let competing = inferred_host_input_paths_for_project(&app, &project);
    assert!(competing.contains(&app.join("vite.config.js")));
    assert!(!competing.contains(&app.join("src/mod255.tsx")));
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn published_application_refusal_observes_only_its_manifest() {
    let scratch =
        std::env::temp_dir().join(format!("host-published-inputs-{}", std::process::id()));
    fs::create_dir_all(&scratch).unwrap();
    fs::write(scratch.join("package.json"), "{\"exports\":\"./index.js\"}").unwrap();
    assert_eq!(
        inferred_host_input_paths_for_project(&scratch, &scratch.join("tsconfig.json")),
        vec![scratch.join("package.json")],
    );
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn admitted_raw_resources_and_absent_lookups_are_daemon_inputs() {
    let scratch = std::env::temp_dir().join(format!("host-raw-inputs-{}", std::process::id()));
    fs::create_dir_all(scratch.join("src")).unwrap();
    let app = fs::canonicalize(&scratch).unwrap();
    fs::write(app.join("package.json"), "{\"private\":true}").unwrap();
    fs::write(app.join("tsconfig.json"), "{}").unwrap();
    fs::write(app.join("vite.config.ts"), "export default {plugins: []};").unwrap();
    fs::write(
        app.join("index.html"),
        "<script type=\"module\" src=\"/src/main.ts\"></script>",
    )
    .unwrap();
    fs::write(
        app.join("src/main.ts"),
        "import './present.css?raw'; import './absent.css?raw';",
    )
    .unwrap();
    let present = app.join("src/present.css");
    let absent = app.join("src/absent.css");
    fs::write(&present, "@reference '../../outside.css';").unwrap();
    let inputs = inferred_host_input_paths_for_project(&app, &app.join("tsconfig.json"));
    assert!(inputs.contains(&present));
    assert!(inputs.contains(&absent));
    let paths = inputs.iter().map(PathBuf::as_path).collect::<Vec<_>>();
    let before = identities(&paths);
    fs::write(&present, "@apply unknown;").unwrap();
    fs::write(&absent, ".x{").unwrap();
    let after = identities(&paths);
    for path in [&present, &absent] {
        let index = inputs.iter().position(|input| input == path).unwrap();
        assert_ne!(before[index], after[index]);
    }
    // Raw acquisition never interprets @reference or observes its target.
    assert!(!inputs.contains(&app.parent().unwrap().join("outside.css")));
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn analyzed_request_projection_matches_extraction() {
    let source = "import type {T} from 'types'; import {value} from 'runtime'; export type {T} from 'other-types'; export {value} from 'reexport'; export * from 'all';";
    let ast = solid_facts::ast::extract("requests.ts", source).unwrap();
    assert_eq!(static_fact_requests(&ast), ["runtime", "reexport", "all"]);
}

#[test]
fn plugin_inventory_is_fingerprinted_without_becoming_an_application_importer() {
    let scratch =
        std::env::temp_dir().join(format!("host-plugin-request-inputs-{}", std::process::id()));
    let implementation = scratch.join("node_modules/audited-plugin/dist/index.js");
    fs::create_dir_all(implementation.parent().unwrap()).unwrap();
    fs::write(&implementation, "import 'plugin-only-request';").unwrap();
    let config =
        host_config::parse(Path::new("vite.config.ts"), "export default {plugins: []};").unwrap();
    let inputs = inferred_host_graph_input_paths(
        &scratch,
        &scratch.join("tsconfig.json"),
        vec![implementation.clone()],
        &config,
        None,
        Some(vec![scratch.clone()]),
        &[],
    );
    assert!(inputs.contains(&implementation));
    assert!(!inputs.iter().any(|path| {
        path.components()
            .any(|part| part.as_os_str() == "plugin-only-request")
    }));
    assert!(!inputs.contains(&scratch.join("node_modules/node_modules/@solidjs/web/package.json")));
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn postcss_discovery_refuses_executable_side_inputs_above_the_leaf() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "solid-host-postcss-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let leaf = scratch.join("app");
    fs::create_dir_all(&leaf).unwrap();
    assert!(side_inputs(&leaf, &mut Vec::new()).is_ok());
    let config = scratch.join("postcss.config.ts");
    fs::write(&config, "import './app/src/App.tsx'; export default {};").unwrap();
    assert!(side_inputs(&leaf, &mut Vec::new()).is_err());
    fs::remove_file(config).unwrap();
    fs::write(
        scratch.join("package.json"),
        "{\"postcss\":{\"plugins\":{\"./app/src/App.tsx\":{}}}}",
    )
    .unwrap();
    assert!(side_inputs(&leaf, &mut Vec::new()).is_err());
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn route_fixture_refuses_exact_server_export_spellings() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir/inferred-host-spa/route-cases.json");
    let cases: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    for case in cases.as_array().unwrap() {
        let ast = solid_facts::ast::extract("route.tsx", case["source"].as_str().unwrap()).unwrap();
        assert_eq!(server_route(&ast), case["server"].as_bool().unwrap());
    }
}

#[test]
fn published_metadata_is_an_open_entry_even_without_private() {
    assert!(unpublished(&serde_json::json!({"name":"app"})));
    for key in ["exports", "main", "module", "bin", "types", "typings"] {
        let mut package = serde_json::json!({"private":true});
        package[key] = serde_json::json!("./dist/index.js");
        assert!(!unpublished(&package), "{key}");
    }
}

#[test]
fn client_probe_order_is_the_installed_integrations_order() {
    let present = [
        "src/entry-server.tsx",
        "src/Document.tsx",
        "src/app.tsx",
        "src/App.js",
        "src/App.tsx",
    ];
    assert_eq!(
        client_root(|path| present.contains(&path)),
        Some(("src/App.tsx".into(), true))
    );
    assert_eq!(
        client_root(|path| present.contains(&path) || path == "src/entry-client.mjs"),
        Some(("src/entry-client.mjs".into(), false))
    );
    assert_eq!(
        client_root(|path| ["src/Document.tsx", "src/entry-server.tsx"].contains(&path)),
        None
    );
    assert_eq!(
        client_root(|path| path == "src/app.jsx"),
        Some(("src/app.jsx".into(), true))
    );
}

#[test]
fn html_module_roots_are_parsed_without_comment_or_inline_roots() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir/inferred-host-reachability");
    assert_eq!(
        html_entries(&fs::read_to_string(fixtures.join("csp-clear.html")).unwrap()),
        Some(vec!["src/completion-cases.ts".into()])
    );
    assert_eq!(
        html_entries(&fs::read_to_string(fixtures.join("csp-denied.html")).unwrap()),
        None
    );
    assert_eq!(
        html_entries(
            "<!doctype html><html><head><title>App</title></head><body><div id='root'></div><script type='module' src='/src/main.ts'></script></body></html>"
        ),
        Some(vec!["src/main.ts".into()])
    );
    for text in [
        "<!-- <script type='module' src='/server.ts'></script> -->",
        "<script type='module'>import '/src/main.ts'</script>",
        "<template><script type='module' src='/src/main.ts'></script></template>",
        "<base href='/server/'><script type='module' src='src/main.ts'></script>",
        "<style>@plugin './src/App.tsx';</style><script type='module' src='/src/main.ts'></script>",
        "<script type='module' src='//remote/app.js'></script>",
        "<script type='module' src='/src/main.ts?server'></script>",
    ] {
        assert_eq!(html_entries(text), None);
    }
}

#[test]
fn passive_svg_does_not_hide_or_create_module_roots() {
    let path = Path::new("index.html");
    let module = "<script type='module' src='/src/main.ts'></script>";
    let passive =
        "<svg><defs><symbol id='shape'><path d='M0 0'/></symbol></defs><use href='#shape'/></svg>";
    assert_eq!(
        html_entries_at(path, &format!("{passive}{module}")).unwrap(),
        vec!["src/main.ts"]
    );
    for foreign in [
        "<svg><script type='module' src='/src/hidden.ts'></script></svg>",
        "<svg><foreignObject><script type='module' src='/src/hidden.ts'></script></foreignObject></svg>",
        "<svg onload='disableModules()'></svg>",
        "<svg><use href='/external.svg#shape'/></svg>",
        "<svg><defs></svg>",
        "<svg><path/></svg><use href='#shape'/>",
    ] {
        let refusal = html_entries_at(path, &format!("\n{foreign}{module}")).unwrap_err();
        assert_eq!(refusal.path.as_path(), path);
        assert_eq!(refusal.line, Some(2));
    }
    let denied = html_entries_at(
        path,
        &format!(
            "\n<meta http-equiv='Content-Security-Policy' content=\"script-src 'none'\">{module}"
        ),
    )
    .unwrap_err();
    assert!(denied.reason.contains("http-equiv"));
    assert_eq!(denied.line, Some(2));
}
