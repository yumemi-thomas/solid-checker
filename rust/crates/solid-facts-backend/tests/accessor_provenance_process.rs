//! Transported source-discovered summaries must follow cross-file edits.
use std::{env, fs, path::PathBuf};

use crate::process_support::temporary_directory;
use solid_facts::compiler::CompilerOptions;
use solid_facts_backend::{NativeIncrementalSession, SourceChange, TypeFactsSession, dialect};

/// The copied fixtures are applications: an open program closes no context
/// provider census, so every build here is a closed program.
fn closed_options() -> solid_reactive_ir::RuleOptions {
    let mut options = solid_reactive_ir::RuleOptions::default();
    options.runtime.program_boundary = Some(solid_reactive_ir::ProgramBoundary::Closed);
    options
}

fn closed_cold(
    facts: &solid_facts::ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
) -> solid_reactive_ir::Program {
    solid_reactive_ir::build_with_accepted_contracts_measured(
        facts,
        dialect,
        &solid_reactive_ir::contract_semantics::AcceptedContractIndex::default(),
        &closed_options(),
    )
    .unwrap()
    .0
}

fn closed_warm(
    incremental: &mut solid_reactive_ir::IncrementalBuilder,
    facts: &solid_facts::ProjectFacts,
    dialect: &dyn solid_dialect::Dialect,
) -> (solid_reactive_ir::Program, solid_reactive_ir::BuildTimings) {
    let (program, timings) = incremental
        .build_with_accepted_contracts_shared(
            facts,
            dialect,
            &solid_reactive_ir::contract_semantics::AcceptedContractIndex::default(),
            &closed_options(),
        )
        .unwrap();
    ((*program).clone(), timings)
}

fn copied_fixture(label: &str, sources: &[&str]) -> PathBuf {
    copied_fixture_named(label, "accessor-provenance", sources)
}

fn copied_fixture_named(label: &str, name: &str, sources: &[&str]) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let fixture = root.join("fixtures/reactive-ir").join(name);
    let directory = temporary_directory(label);
    for name in ["tsconfig.json", "solid-js.d.ts"]
        .into_iter()
        .chain(sources.iter().copied())
    {
        fs::copy(fixture.join(name), directory.join(name)).unwrap();
    }
    for package in ["solid-js", "@solidjs/signals", "@solidjs/web"] {
        let stub = directory.join("node_modules").join(package);
        fs::create_dir_all(&stub).unwrap();
        fs::copy(
            fixture
                .join("node_modules")
                .join(package)
                .join("package.json"),
            stub.join("package.json"),
        )
        .unwrap();
    }
    directory
}

#[test]
fn returned_array_edits_match_cold_in_both_directions() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = copied_fixture_named(
        "array-provenance-edit",
        "provider-array-provenance",
        &["factory.ts"],
    );
    let source = "import { makeProviders } from './factory';\nexport function Body() {\n const providers = makeProviders();\n const values = providers.map(p => p.query('body'));\n return <p>{values.join()}</p>;\n}\n";
    fs::write(directory.join("App.tsx"), source).unwrap();
    fs::write(directory.join("fakejs.ts"), "import type { Signal } from 'solid-js';\nexport type { SourceAccessor } from 'solid-js';\nexport declare function createSignal<T>(value: T): Signal<T>;\n").unwrap();
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let caller = directory
        .join("App.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let factory = directory
        .join("factory.ts")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let original = fs::read_to_string(&factory).unwrap();
    let without_read = original.replace("return first();", "return 0      ;");
    assert_ne!(original, without_read);
    assert_eq!(original.len(), without_read.len());
    let without_origin = original.replace("\"solid-js\"", "\"./fakejs\"");
    assert_eq!(
        original.len(),
        without_origin.len(),
        "keep the selected arrows' spans unchanged"
    );
    let vocabulary = dialect::default_dialect().vocabulary;
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (program, _) = closed_warm(&mut incremental, &first, vocabulary);
    let consumer_read = |program: &solid_reactive_ir::Program| {
        program.reads.iter().any(|read| {
            read.location.path.as_ref() == caller
                && read.accessor.as_ref() == "first"
                && read.transported
                && !read.summary_attributed
                && !read.is_uncertifiable()
        })
    };
    assert!(
        consumer_read(&program),
        "the literal array must supply a proven consumer read: {:?}",
        program.reads
    );
    let mut version = 0;
    let mut check = |path: &str, source: String, reads: bool| {
        version += 1;
        let facts = session
            .edit(
                vec![SourceChange {
                    path: path.into(),
                    version,
                    source: Some(source),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let cold = closed_cold(&facts, vocabulary);
        let (warm, _) = closed_warm(&mut incremental, &facts, vocabulary);
        assert_eq!(consumer_read(&cold), reads, "array transition {version}");
        assert_eq!(
            warm, cold,
            "array transport or summary dependency, edit {version}"
        );
    };
    check(&factory, without_read, false);
    check(&factory, original.clone(), true);
    for mutation in [
        "providers[0] = { query: () => 0 };",
        "providers.length = 0;",
        "providers.push({ query: () => 0 });",
        "providers.pop();",
        "providers.splice(0, 1);",
        "providers.shift();",
        "providers.unshift({ query: () => 0 });",
        "providers.sort();",
        "providers.reverse();",
        "providers.fill({ query: () => 0 });",
        "providers.copyWithin(0, 1);",
        "Object.assign(providers, { 0: {query: () => 0} });",
        "Object.defineProperty(providers, '0', {value: {query: () => 0}});",
        "const alias = providers; void alias;",
    ] {
        let mutated = source.replace(" const values", &format!(" {mutation}\n const values"));
        check(&caller, mutated, false);
        check(&caller, source.into(), true);
    }
    // Isolated positional-spread wildcard: neither another wildcard nor an
    // escaped array binding can mask whether the common builtin guard ran.
    for mutation in [
        "const pair: [object, string] = [{}, 'query']; Reflect.set(...pair, 'replacement');",
        "Object.defineProperty({}, `query`, { value: () => 0 });",
        "const args: [object, object] = [{}, { query: () => 0 }]; Object.assign(...args);",
        "const other = {query: () => 0}; other['query' as const] = () => 0;",
    ] {
        check(&factory, format!("{original}\n{mutation}\n"), false);
        check(&factory, original.clone(), true);
    }
    check(&factory, without_origin, false);
    check(&factory, original, true);
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn transported_member_helper_edits_match_cold_in_both_directions() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = copied_fixture(
        "transport-summary-edit",
        &["cache-helper.ts", "cache-factory.ts", "Cache.tsx"],
    );
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let helper = directory
        .join("cache-helper.ts")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let caller = directory
        .join("Cache.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let original = fs::read_to_string(&helper).unwrap();
    let without_read = original.replace("void count()", "void plain()");
    assert_ne!(without_read, original);
    assert_eq!(
        without_read.len(),
        original.len(),
        "hold declaration and caller spans stable"
    );
    let vocabulary = dialect::default_dialect().vocabulary;
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (program, _) = closed_warm(&mut incremental, &first, vocabulary);
    let consumer_read = |program: &solid_reactive_ir::Program| {
        program.reads.iter().any(|read| {
            read.location.path.as_ref() == caller
                && read.transported
                && !read.summary_attributed
                && read.execution == solid_reactive_ir::ExecutionRole::UntrackedRendering
        })
    };
    assert!(
        consumer_read(&program),
        "the member must be admitted and carry a proven body read"
    );
    let unrelated_write =
        format!("{original}\nconst other = {{ ready: () => 0 }}; other.ready = () => 0;\n");
    let unknown_assign =
        format!("{original}\nconst other = {{ plain: 0 }}; Object.assign({{}}, other);\n");
    let spread_assign =
        format!("{original}\nconst other = {{ plain: 0 }}; Object.assign({{}}, {{ ...other }});\n");
    let first_slot_assign = format!(
        "{original}\nconst args: [object, object] = [{{}}, {{ ready: () => 0 }}]; Object.assign(...args);\n"
    );
    let later_slot_assign = format!(
        "{original}\nconst sources: object[] = [{{ ready: () => 0 }}]; Object.assign({{}}, ...sources);\n"
    );
    let wrapped_write = format!(
        "{original}\nconst other = {{ ready: () => 0 }}; other[\"ready\" as const] = () => 0;\n"
    );
    let wrapped_delete = format!(
        "{original}\nconst other: {{ ready?: () => number }} = {{}}; delete other[\"ready\"!];\n"
    );
    let wrapped_descriptor = format!(
        "{original}\nObject.defineProperties({{}}, {{ [\"ready\" satisfies string]: {{ value: () => 0 }} }});\n"
    );
    let template_write =
        format!("{original}\nconst other = {{ ready: () => 0 }}; other[`ready`] = () => 0;\n");
    for (version, source, reads) in [
        (1, without_read, false),
        (2, original.clone(), true),
        (3, unrelated_write, false),
        (4, original.clone(), true),
        (5, unknown_assign, false),
        (6, original.clone(), true),
        (7, spread_assign, false),
        (8, original.clone(), true),
        (9, first_slot_assign, false),
        (10, original.clone(), true),
        (11, later_slot_assign, false),
        (12, original.clone(), true),
        (13, wrapped_write, false),
        (14, original.clone(), true),
        (15, wrapped_delete, false),
        (16, original.clone(), true),
        (17, wrapped_descriptor, false),
        (18, original.clone(), true),
        (19, template_write, false),
        (20, original, true),
    ] {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: helper.clone(),
                    version,
                    source: Some(source),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        assert!(session.last_build_timings().source_files_reused > 0);
        let cold = closed_cold(&facts, vocabulary);
        let (warm, timings) = closed_warm(&mut incremental, &facts, vocabulary);
        assert!(
            timings.source_discovery_reused_files > 0,
            "exercise reuse of unchanged caller/factory facts"
        );
        assert_eq!(
            consumer_read(&cold),
            reads,
            "the helper summary or unrelated member-name write must change transport"
        );
        assert_eq!(
            warm, cold,
            "transported target dependency must follow the helper summary, edit {version}"
        );
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn context_intermediate_handoff_edits_match_cold_in_both_directions() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = copied_fixture_named(
        "context-handoff-edit",
        "context-member-forwarding",
        &["context.tsx", "App.tsx"],
    );
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let context = directory
        .join("context.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let original = fs::read_to_string(&context).unwrap();
    let aliased = original.replace(
        "return value;",
        "const alias = value; void alias; return value;",
    );
    let guarded = original.replace(
        "return value;",
        "if (!value) throw new Error('missing'); return value;",
    );
    let written = format!(
        "{original}\nconst unrelated = {{ orientation: () => 0 }}; unrelated.orientation = () => 0;\n"
    );
    let vocabulary = dialect::default_dialect().vocabulary;
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (program, _) = closed_warm(&mut incremental, &first, vocabulary);
    let consumer_read = |program: &solid_reactive_ir::Program| {
        program.reads.iter().any(|read| {
            read.context.as_ref() == "Body"
                && read.transported
                && !read.summary_attributed
                && !read.is_uncertifiable()
        })
    };
    assert!(consumer_read(&program));
    for (version, source, reads) in [
        (1, aliased, false),
        (2, original.clone(), true),
        (3, guarded, true),
        (4, original.clone(), true),
        (5, written, false),
        (6, original, true),
    ] {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: context.clone(),
                    version,
                    source: Some(source),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let cold = closed_cold(&facts, vocabulary);
        let (warm, _) = closed_warm(&mut incremental, &facts, vocabulary);
        assert_eq!(
            consumer_read(&cold),
            reads,
            "context handoff transition {version}"
        );
        assert_eq!(
            warm, cold,
            "context handoff census dependency, edit {version}"
        );
    }
    let caller = directory
        .join("App.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let source = fs::read_to_string(&caller).unwrap();
    let mut version = 6;
    for refused in [
        source.replace(
            "orientation: readOrientation }",
            "orientation: readOrientation = () => 0 }",
        ),
        source
            .replace(
                "orientation: readOrientation }",
                "orientation: readOrientation, ...rest }",
            )
            .replace(
                "const n = readOrientation();",
                "void rest; const n = readOrientation();",
            ),
        source.replace(
            "const n = readOrientation();",
            "const alias = readOrientation; void alias; const n = readOrientation();",
        ),
        source.replace(
            "const { orientation } = value;",
            "const { orientation } = value; const alias = orientation; void alias;",
        ),
        source.replace(
            "orientation: readOrientation }",
            "\"orientation\": readOrientation }",
        ),
        source.replace(
            "orientation: readOrientation }",
            "orient\\u0061tion: readOrientation }",
        ),
    ] {
        for (changed, expected) in [(refused, false), (source.clone(), true)] {
            version += 1;
            let facts = session
                .edit(
                    vec![SourceChange {
                        path: caller.clone(),
                        version,
                        source: Some(changed),
                        compiler_options: CompilerOptions::default(),
                    }],
                    None,
                )
                .unwrap();
            let cold = closed_cold(&facts, vocabulary);
            let (warm, _) = closed_warm(&mut incremental, &facts, vocabulary);
            assert_eq!(
                consumer_read(&cold),
                expected,
                "destructured context transition {version}"
            );
            assert_eq!(
                warm, cold,
                "destructured context census dependency, edit {version}"
            );
        }
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn returned_tuple_iteration_edits_match_cold_in_both_directions() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = copied_fixture_named(
        "tuple-iteration-edit",
        "tuple-iteration-veto",
        &["factory.ts", "App.tsx", "mutations.ts", "gateways.ts"],
    );
    fs::write(directory.join("mutations.ts"), "export {};\n").unwrap();
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let mutation = directory
        .join("mutations.ts")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let caller = directory
        .join("App.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let vocabulary = dialect::default_dialect().vocabulary;
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let has_tuple_read = |program: &solid_reactive_ir::Program| {
        program.reads.iter().any(|read| {
            read.location.path.as_ref() == caller
                && read.transported
                && !read.summary_attributed
                && read.execution == solid_reactive_ir::ExecutionRole::UntrackedRendering
        })
    };
    let first = session.analyze().unwrap();
    assert!(has_tuple_read(
        &closed_warm(&mut incremental, &first, vocabulary).0
    ));
    for (version, source, reads) in [
        (
            1,
            "(Array.prototype as any)[Symbol.iterator] = function* () { yield () => 'x'; yield () => 'y'; };",
            false,
        ),
        (2, "(Array.prototype as any).zzz = () => 'x';", true),
        (
            3,
            "const IP = Object.getPrototypeOf([][Symbol.iterator]()); IP.next = () => ({ value: () => 'x', done: false });",
            false,
        ),
        (4, "", true),
        (
            5,
            "(globalThis.Function as any)(\"globalThis.Proxy = class { constructor() { Array.prototype[Symbol.iterator] = function*(){yield ()=> 'x'; yield ()=> 'y';}; } }\")(); new Proxy(Array.prototype, {});",
            false,
        ),
        (6, "new Proxy(Array.prototype, {});", true),
        (
            7,
            "const { Function: F }: any = globalThis; F(\"Function.prototype.call = function() {};\")(); function noop() {} noop.call(undefined);",
            false,
        ),
        (8, "function noop() {} noop.call(undefined);", true),
        (
            9,
            "function gateway(): any { return globalThis.Function; } gateway()(\"Array.prototype[Symbol.iterator] = function*(){yield ()=> 'x'; yield ()=> 'y';};\")();",
            false,
        ),
        (10, "", true),
        (
            11,
            "function gateway(f: any): any { return f; } gateway(globalThis.Function)(\"Array.prototype[Symbol.iterator] = function*(){yield ()=> 'x'; yield ()=> 'y';};\")();",
            false,
        ),
        (12, "", true),
        (
            13,
            "import { F } from './gateways.ts'; F(\"Array.prototype[Symbol.iterator] = function*(){yield ()=> 'x'; yield ()=> 'y';};\")();",
            false,
        ),
        (14, "", true),
    ] {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: mutation.clone(),
                    version,
                    source: Some(format!("{source}\nexport {{}};\n")),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        assert!(session.last_build_timings().source_files_reused > 0);
        let cold = closed_cold(&facts, vocabulary);
        let warm = closed_warm(&mut incremental, &facts, vocabulary).0;
        assert_eq!(has_tuple_read(&cold), reads, "edit {version}");
        assert_eq!(warm, cold, "tuple iteration edit {version}");
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn single_call_parameter_edits_match_cold_in_both_directions() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = copied_fixture_named(
        "parameter-flow-edit",
        "provider-array-provenance",
        &["factory.ts"],
    );
    let source = "import { makeProviders, type Provider } from './factory';\nfunction queryProviders(providers: Provider[]) { return providers.map(p => p.query('body')); }\nexport function Body() { const providers = makeProviders(); const values = queryProviders(providers); return <p>{values.join()}</p>; }\n";
    fs::write(directory.join("App.tsx"), source).unwrap();
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let caller = directory
        .join("App.tsx")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let vocabulary = dialect::default_dialect().vocabulary;
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let first = session.analyze().unwrap();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let (program, _) = closed_warm(&mut incremental, &first, vocabulary);
    let consumer_read = |program: &solid_reactive_ir::Program| {
        program.reads.iter().any(|read| {
            read.location.path.as_ref() == caller
                && read.context.as_ref() == "Body"
                && read.transported
                && !read.summary_attributed
                && !read.is_uncertifiable()
        })
    };
    assert!(
        consumer_read(&program),
        "one exact handoff must carry the consumer read: {:?}",
        program.reads
    );
    let mut version = 0;
    let mut check = |changed: String, expected: bool| {
        version += 1;
        let facts = session
            .edit(
                vec![SourceChange {
                    path: caller.clone(),
                    version,
                    source: Some(changed),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let cold = closed_cold(&facts, vocabulary);
        let (warm, _) = closed_warm(&mut incremental, &facts, vocabulary);
        assert_eq!(
            consumer_read(&cold),
            expected,
            "parameter transition {version}"
        );
        assert_eq!(
            warm, cold,
            "parameter handoff/census dependency, edit {version}"
        );
    };
    for refused in [
        source.replace("return <p>", "void queryProviders(providers); return <p>"),
        source.replace("function queryProviders", "export function queryProviders"),
        source.replace(
            "{ return providers.map",
            "{ providers = []; return providers.map",
        ),
        source.replace("providers: Provider[]", "providers: Provider[] = []"),
        source.replace(
            "{ return providers.map",
            "{ void arguments; return providers.map",
        ),
        source.replace(
            "const values = queryProviders(providers)",
            "const alias = queryProviders; const values = alias(providers)",
        ),
        source.replace(
            "{ return providers.map",
            "{ const alias = providers; void alias; return providers.map",
        ),
        source.replace(
            "{ return providers.map",
            "{ providers.pop(); return providers.map",
        ),
        source.replace(
            "queryProviders(providers); return",
            "queryProviders(...[providers] as [Provider[]]); return",
        ),
        format!(
            "{source}\ndeclare function retain(fn: typeof queryProviders): void; retain(queryProviders);\n"
        ),
        format!(
            "{source}\nconst pair: [object, string] = [{{}}, 'query']; Reflect.set(...pair, 'replacement');\n"
        ),
        format!(
            "{source}\nconst other = {{ query: () => 0 }}; other['query' as const] = () => 0;\n"
        ),
    ] {
        check(refused, false);
        check(source.into(), true);
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn context_receiver_census_edits_match_cold_in_both_directions() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let directory = copied_fixture(
        "transport-census-edit",
        &["ContextStability.tsx", "context-mutation.ts"],
    );
    let project = directory
        .join("tsconfig.json")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let mutation = directory
        .join("context-mutation.ts")
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let controls = directory.join("ContextStability.tsx");
    let source = fs::read_to_string(&controls).unwrap();
    fs::write(
        &controls,
        source
            .replace("replaced.stale = () => 0;", "void replaced.stale();")
            .replace(
                "useContext(Unbound).stale = () => 0;",
                "void useContext(Unbound).stale();",
            ),
    )
    .unwrap();
    let original = fs::read_to_string(&mutation).unwrap();
    let closed = original.replace("replaced.stale = () => 0;", "void replaced.stale();");
    assert_ne!(closed, original);
    let vocabulary = dialect::default_dialect().vocabulary;
    let typescript = TypeFactsSession::open(&typefacts, &project, &[]).unwrap();
    let (mut session, _) =
        NativeIncrementalSession::open_pipelined(dialect::default_dialect(), project, typescript)
            .unwrap();
    let first = session.analyze().unwrap();
    // An open program closes no context provider census; this census is
    // the application's.
    let mut options = solid_reactive_ir::RuleOptions::default();
    options.runtime.program_boundary = Some(solid_reactive_ir::ProgramBoundary::Closed);
    let contracts = solid_reactive_ir::contract_semantics::AcceptedContractIndex::default();
    let mut incremental = solid_reactive_ir::IncrementalBuilder::default();
    let program = incremental
        .build_with_accepted_contracts_shared(&first, vocabulary, &contracts, &options)
        .unwrap()
        .0;
    let consumer_read = |program: &solid_reactive_ir::Program| {
        program.reads.iter().any(|read| {
            read.context.as_ref() == "CrossConsumerMutation"
                && read.transported
                && !read.summary_attributed
                && !read.is_uncertifiable()
        })
    };
    assert!(!consumer_read(&program));
    let callback_mutation = |body: &str| {
        format!(
            "{closed}\nconst useCross = () => useContext(CrossAccount);\n\
             function replace(read: () => {{ stale: () => number }}) {{ {body} }}\n\
             replace(useCross);\n"
        )
    };
    let first_slot = callback_mutation(
        "const args: [{ stale: () => number }, { stale: () => number }] = \
         [read(), { stale: () => 0 }]; Object.assign(...args);",
    );
    let wrapped_write = callback_mutation("read()[\"stale\" as const] = () => 0;");
    let wrapped_descriptor = callback_mutation(
        "Object.defineProperties(read(), { [\"stale\" as const]: { value: () => 0 } });",
    );
    for (version, source, reads) in [
        (1, closed.clone(), true),
        (2, original, false),
        (3, closed.clone(), true),
        (4, first_slot, false),
        (5, closed.clone(), true),
        (6, wrapped_write, false),
        (7, closed.clone(), true),
        (8, wrapped_descriptor, false),
        (9, closed, true),
    ] {
        let facts = session
            .edit(
                vec![SourceChange {
                    path: mutation.clone(),
                    version,
                    source: Some(source),
                    compiler_options: CompilerOptions::default(),
                }],
                None,
            )
            .unwrap();
        let cold = solid_reactive_ir::build_with_accepted_contracts_measured(
            &facts, vocabulary, &contracts, &options,
        )
        .unwrap()
        .0;
        let warm = (*incremental
            .build_with_accepted_contracts_shared(&facts, vocabulary, &contracts, &options)
            .unwrap()
            .0)
            .clone();
        assert_eq!(
            consumer_read(&cold),
            reads,
            "the shared receiver census must change transport"
        );
        assert_eq!(
            warm, cold,
            "result reuse must follow the complete context census, edit {version}"
        );
    }
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}
