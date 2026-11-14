//! Batch 11, slice 9B (ADR 0973): every read and call of a project function written after an async function's own suspension ends, or is uncertifiable.
//!
//! `after_await_accounting.rs` lists the sites (the reads of the universe and the calls written after their async function's own suspension) and each must end in a
//! row of SC1002, an SC9012, the compiler's discard or a typed reason; one that ends in none gets an uncertifiable row of SC1002, blocker
//! `read-after-await-unaccounted`, and never a violation.
//!
//! The mutation: `SOLID_CHECKER_IR_ABLATE_SILENCE=<id>` drops one reason, and the sites it ended must then each carry the row, with no other answer moved.
use std::{collections::BTreeSet, env, fs, path::Path, process::Command};

const BLOCKER: &str = "read-after-await-unaccounted";
const NEVER_RUNS: &str = "after_await_accounting.account.never-runs";
const LIBRARY_RUNS_NOTHING: &str = "after_await_accounting.account.library-runs-nothing";
const OWNER_CAPTURED: &str = "after_await_accounting.account.owner-captured";

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/reactive-ir")
        .join(name)
}

/// The findings of a one-shot check of the fixture `name`, with `ablate` the reasons to drop.
fn check(typefacts: &str, name: &str, ablate: Option<&str>) -> Vec<serde_json::Value> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_solid-checker-rust"));
    command
        .env("SOLID_TYPEFACTS_BIN", typefacts)
        .env("SOLID_CHECKER_DAEMON", "0")
        .env_remove("SOLID_CHECKER_IR_ABLATE_SILENCE")
        .args(["--format", "json", "--project"])
        .arg(fixture(name).join("tsconfig.json"));
    // The runtime the fixture declares (`.solid-checker/runtime.json`, as `scripts/coverage.mjs` reads it).
    if let Ok(text) = fs::read_to_string(fixture(name).join(".solid-checker/runtime.json")) {
        let runtime: serde_json::Value = serde_json::from_str(&text).unwrap();
        for (key, flag) in [
            ("target", "--runtime-target"),
            ("build", "--runtime-build"),
            ("rendering", "--rendering"),
        ] {
            if let Some(value) = runtime[key].as_str() {
                command.args([flag, value]);
            }
        }
    }
    if let Some(ablate) = ablate {
        command.env("SOLID_CHECKER_IR_ABLATE_SILENCE", ablate);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success() || output.status.code() == Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    report["findings"].as_array().unwrap().clone()
}

/// A finding as a comparable string: code, kind, place, blocker.
fn key(finding: &serde_json::Value) -> String {
    format!(
        "{} {} {}:{}:{} {}",
        finding["id"],
        finding["kind"],
        finding["primaryLocation"]["path"],
        finding["primaryLocation"]["line"],
        finding["primaryLocation"]["column"],
        finding["analysisContext"]
    )
}

/// The start bytes of the rows the pass files: SC1002, blocker `read-after-await-unaccounted`. Each is uncertifiable, never a violation.
fn unaccounted(findings: &[serde_json::Value]) -> BTreeSet<u64> {
    findings
        .iter()
        .filter(|finding| finding["analysisContext"] == BLOCKER)
        .map(|finding| {
            assert_eq!(finding["id"], "SC1002", "the row is SC1002's");
            assert_eq!(finding["kind"], "uncertifiable", "never a violation");
            finding["primaryLocation"]["startByte"].as_u64().unwrap()
        })
        .collect()
}

/// Where `skip` ends inside the first occurrence of `anchor` in `source`.
fn at(source: &str, anchor: &str, skip: &str) -> u64 {
    let found = source
        .find(anchor)
        .unwrap_or_else(|| panic!("no `{anchor}`"));
    (found + skip.len()) as u64
}

fn source(name: &str) -> String {
    fs::read_to_string(fixture(name).join("App.tsx")).unwrap()
}

/// Every answer of the named run is still there in the ablated run, and what is new is exactly `added`.
fn only_these_moved(
    named: &[serde_json::Value],
    ablated: &[serde_json::Value],
    added: &BTreeSet<u64>,
) {
    let before = named.iter().map(key).collect::<BTreeSet<_>>();
    let after = ablated.iter().map(key).collect::<BTreeSet<_>>();
    assert!(before.is_subset(&after), "an answer was lost");
    let new = ablated
        .iter()
        .filter(|finding| !before.contains(&key(finding)))
        .map(|finding| {
            assert_eq!(
                finding["analysisContext"], BLOCKER,
                "an answer other than the unaccounted rows moved"
            );
            finding["primaryLocation"]["startByte"].as_u64().unwrap()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(&new, added, "the rows the dropped reason ended");
}

#[test]
fn a_site_no_reason_ends_gets_sc1002s_uncertifiable_row() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let name = "after-await-universe";
    let source = source(name);
    let rows = unaccounted(&check(&typefacts, name, None));
    // A read after an await in an async function no computation runs, a call of a project function whose closure is not silent, a call of a function value.
    for (anchor, skip, what) in [
        (
            "count();\n      }}\n    >",
            "",
            "a read in an event handler",
        ),
        (
            "return count();\n}\n\n// A call",
            "return ",
            "a read in a helper",
        ),
        (
            "stringify(1);\n  readsCount",
            "",
            "a callee that calls the standard library",
        ),
        ("readsCount();\n  ping", "", "a callee that reads a source"),
        ("ping(3);\n}", "", "a callee that calls itself"),
        (
            "helpers.format(1);",
            "",
            "a namespace member whose closure calls the standard library",
        ),
        (
            "codec.encode(1);",
            "",
            "an exact member whose closure calls the standard library",
        ),
        (
            "meter.read();",
            "",
            "an exact member whose closure reads a source",
        ),
        ("this.helper();", "", "an inexact member: `this`"),
        (
            "loader.helper();",
            "",
            "an inexact member: a parameter typed as the class",
        ),
        ("callback();\n}", "", "a function value"),
        ("reassigned();\n}", "", "a reassigned `let`"),
        ("count();\n    void item", "", "a read after a `for await`"),
        // The review of #233, B1: a call keeps its row whatever its callee's closure reads (there is no callee reason).
        (
            "peekAlias();",
            "",
            "a callee that reads a store through an alias",
        ),
        (
            "peekStored();",
            "",
            "a callee that steps a stored generator with a spread",
        ),
        (
            "peekProxy();",
            "",
            "a callee that reaches a trap installed by reference",
        ),
        (
            "peekDescriptor();",
            "",
            "a callee that reaches a getter installed by reference",
        ),
        // B2: a method of a module-scope literal the receiver census withholds, its computed and optional members, an arrow property.
        (
            "registry.run();",
            "",
            "a method of a module-scope object literal",
        ),
        (
            "registry[\"run\"]();",
            "",
            "a computed member of a module-scope literal",
        ),
        (
            "registry?.run();",
            "",
            "an optional member of a module-scope literal",
        ),
        (
            "registry.tick();",
            "",
            "an arrow property of a module-scope literal",
        ),
        // B3: a standard call that runs a function value, and a function literal called where it is written.
        ("reader.call(undefined);", "", "`f.call(…)`"),
        ("reader.apply(undefined, []);", "", "`f.apply(…)`"),
        (
            "Reflect.apply(reader, undefined, []);",
            "",
            "`Reflect.apply(f, …)`",
        ),
        (
            "(() => count())();",
            "(",
            "an arrow called where it is written",
        ),
        (
            "(function () {\n    return count();",
            "(",
            "a function expression called where it is written",
        ),
        (
            "await (async () => count())();",
            "await (",
            "an async arrow called where it is written",
        ),
        // ADR 0976, S2 and R1 to R4 of the reviews of #233: a call into the standard library, a setter or a construct is a site. This program defines a property reflectively (the
        // B1 shapes), so no call of it is proven inert: `Outside`'s calls keep their rows too (the program leaves a built-in alone only in `after-await-library`).
        (
            "[1].forEach(() => {\n    count();",
            "",
            "a callback the standard library runs here",
        ),
        (
            "Array.from([1], reader);",
            "",
            "a project function handed to the standard library as its callback",
        ),
        (
            "new Promise<number>((done) => done(count()));",
            "",
            "a Promise executor",
        ),
        ("setCount(() => count());", "", "a setter's updater"),
        (
            "setCount(reader);",
            "",
            "a project function handed to a setter",
        ),
        (
            "target.dispatchEvent(new Event(\"x\"));",
            "",
            "a synchronous dispatch to a listener registered earlier",
        ),
        ("button.click();", "", "`click()`"),
        (
            "controller.abort();",
            "",
            "`abort()`: no resolved-call fact, the symbol is the library's",
        ),
        (
            "stored.next(1);",
            "",
            "a stored generator stepped with an argument",
        ),
        (
            "iterator.next(undefined);",
            "",
            "an iterator literal under a library type",
        ),
        ("counts.get(\"x\");", "", "a Map subclass's method"),
        ("new Reads();", "", "a class whose field initializer reads"),
        (
            "new Wraps(reader);",
            "",
            "a class whose constructor calls its argument",
        ),
        // R5: `runWithOwner` whose owner is not proven captured before the suspension, through a namespace import and an alias, and a project function of the same name.
        (
            "runWithOwner(null, () => count());\n  runWithOwner(getOwner()",
            "",
            "a null owner",
        ),
        (
            "runWithOwner(getOwner(), () => count());",
            "",
            "an owner taken after the suspension",
        ),
        (
            "runWithOwner(unproven, () => count());",
            "",
            "an owner no fact proves",
        ),
        (
            "solid.runWithOwner(null, () => count());",
            "",
            "a null owner, through a namespace import",
        ),
        (
            "withOwner(null, () => count());",
            "",
            "a null owner, through an alias import",
        ),
        (
            "await pause();\n  runWithOwner(owner, () => count());",
            "await pause();\n  ",
            "a project function named `runWithOwner`: a call of a project function, no primitive's reason ends it",
        ),
        // The same calls as `after-await-library`'s `Outside`, in a program that touches a built-in.
        (
            "JSON.stringify({ n: 1 });",
            "",
            "an inert call in a program that defines a property reflectively",
        ),
        (
            "Math.max(1, 2);",
            "",
            "an inert call in a program that defines a property reflectively",
        ),
    ] {
        let start = at(&source, anchor, skip);
        assert!(rows.contains(&start), "{what}: no SC1002 row at {start}");
    }
}

#[test]
fn the_never_runs_reason_ends_exactly_its_sites_and_no_callee_reason_ends_a_call() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let name = "after-await-universe-none";
    let source = source(name);
    let named = check(&typefacts, name, None);
    // The calls of pure project functions (the whole closure resolves exactly and reads no source) keep their row: there is no callee reason (the review of #233, B1: the
    // closure is a list of reads and calls, and a read reaches a continuation through values no call shows). They are the only rows of the program.
    let pure = BTreeSet::from([
        at(&source, "double(2);", ""),
        at(&source, "quad(2);", ""),
        at(&source, "helpers.triple(2);", ""),
        at(&source, "calc.halve(4);", ""),
    ]);
    assert_eq!(
        unaccounted(&named),
        pure,
        "the pure callees keep their row, and nothing else is unaccounted"
    );
    // The reason: no execution of the program runs the site. Dropping it files exactly those sites.
    let never = BTreeSet::from([
        at(&source, "stringify(1);", ""),
        at(&source, "stringify(2);", ""),
    ]);
    let ablated = check(&typefacts, name, Some(NEVER_RUNS));
    only_these_moved(&named, &ablated, &never);
}

#[test]
fn a_program_without_a_recognized_computation_keeps_every_sites_row() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let name = "after-await-universe-census";
    let source = source(name);
    let named = check(&typefacts, name, None);
    let rows = unaccounted(&named);
    // ADR 0973: there is no census reason. The program holds no tracked async computation the producers find, and every read written after an await keeps its row: the
    // functions the program runs itself, the exported one, and `work`, a function declaration handed by identifier to a wrapper that gives it to `createMemo` (Chrome
    // runs it as a computation: the oracle case `after-await-universe-census-wrapper`).
    for (anchor, skip, what) in [
        (
            "async function load() {\n  await pause();\n  return count()",
            "async function load() {\n  await pause();\n  return ",
            "a read in a function the program calls at module level",
        ),
        (
            "await pause();\n  count();\n  return load()",
            "await pause();\n  ",
            "a read in a function that calls another",
        ),
        (
            "return load();",
            "return ",
            "a call of a project function whose closure reads a source",
        ),
        (
            "export async function exported() {\n  await pause();\n  return count()",
            "export async function exported() {\n  await pause();\n  return ",
            "a read in an exported function",
        ),
        (
            "async function work() {\n  await pause();\n  return count()",
            "async function work() {\n  await pause();\n  return ",
            "a read in a function declaration handed to a wrapper that gives it to `createMemo`",
        ),
        (
            "return peek();",
            "return ",
            "a call of a project function whose closure reads no listed source (there is no callee reason, with or without a recognized computation)",
        ),
    ] {
        let start = at(&source, anchor, skip);
        assert!(rows.contains(&start), "{what}: no SC1002 row at {start}");
    }
    // No typed reason holds at any of these sites: dropping it moves nothing.
    let ablated = check(&typefacts, name, Some(NEVER_RUNS));
    only_these_moved(&named, &ablated, &BTreeSet::new());
}

/// The calls of `after-await-library`'s `Outside`: each is a standard-library call, a setter or a construct whose receiver and arguments are proven inert, in a program that
/// leaves every built-in alone (ADR 0976).
fn inert_calls(source: &str) -> BTreeSet<u64> {
    BTreeSet::from([
        at(source, "JSON.stringify({ n: 1 });", ""),
        at(source, "Promise.resolve(1);", ""),
        at(source, "setCount(1);", ""),
        at(source, "setFlag(1);", ""),
        at(source, "new Date();", ""),
        at(source, "Math.max(1, 2);", ""),
        at(source, "Object.keys({ a: 1 });", ""),
        at(source, "setTimeout(() => {\n    count();\n  }, 0);", ""),
    ])
}

#[test]
fn a_library_call_that_runs_no_project_code_is_no_site() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let name = "after-await-library";
    let source = source(name);
    let rows = unaccounted(&check(&typefacts, name, None));
    // Each call of `Outside` is an exactly resolved standard-library call, a setter or a construct whose receiver and arguments are proven inert, in a program that leaves every
    // built-in alone: it ends in no row. This pins the one typed fact that ends a library call, so it does not grow.
    for start in inert_calls(&source) {
        assert!(!rows.contains(&start), "an inert call has a row at {start}");
    }
    // The same calls with one thing the proof lacks keep their row: a value no fact proves, a function-valued `equals`, options no fact shows, a spread, a receiver that is only a
    // library type, a call with no resolved-call fact.
    for (anchor, what) in [
        ("JSON.stringify(value);", "an argument no fact proves"),
        ("Promise.resolve(value);", "an argument no fact proves"),
        (
            "setCount(value as number);",
            "an argument no fact proves, behind an assertion",
        ),
        ("setCustom(1);", "a signal whose `equals` is a function"),
        ("setNamed(1);", "a signal whose options no fact shows"),
        (
            "new Date(text);",
            "a construct given a value no fact proves",
        ),
        ("Math.max(...list);", "a spread argument"),
        ("list.push(1);", "a receiver that only has a library type"),
        (
            "text.trim();",
            "an argumentless call: no resolved-call fact",
        ),
    ] {
        let start = at(&source, anchor, "");
        assert!(rows.contains(&start), "{what}: no SC1002 row at {start}");
    }
}

#[test]
fn the_library_reason_ends_exactly_the_inert_calls_and_one_written_builtin_takes_it_from_all() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let name = "after-await-library";
    let source = source(name);
    let named = check(&typefacts, name, None);
    // The mutation test: dropping `library-runs-nothing` files exactly the calls it ended, and moves no other answer.
    let ablated = check(&typefacts, name, Some(LIBRARY_RUNS_NOTHING));
    only_these_moved(&named, &ablated, &inert_calls(&source));
    // The twin writes `String.prototype.concat`: every call of `Outside` keeps its row, and the reason ends none of them.
    let name = "after-await-library-patched";
    let patched = self::source(name);
    let named = check(&typefacts, name, None);
    let rows = unaccounted(&named);
    for (anchor, skip) in [
        ("JSON.stringify({ n: 1 });", ""),
        ("Promise.resolve(1);", ""),
        ("new Date();", ""),
        ("Math.max(1, 2);", ""),
        ("Object.keys({ a: 1 });", ""),
        ("setTimeout(() => {\n    count();\n  }, 0);", ""),
        ("setTimeout(\"count()\", 0);", ""),
    ] {
        let start = at(&patched, anchor, skip);
        assert!(
            rows.contains(&start),
            "`{anchor}`: a written built-in leaves it a row, found none at {start}"
        );
    }
    // The setter has an answer of its own here: the program writes `String.prototype.concat`, so the tuple destructuring of `createSignal` is an iteration the program may have
    // rewritten, and its call is already an SC9012 row (`tuple-iteration-written`): never clean, whatever the library reason says.
    let setter = at(&patched, "setCount(1);", "");
    assert!(
        named
            .iter()
            .any(|finding| finding["kind"] == "uncertifiable"
                && finding["primaryLocation"]["startByte"].as_u64() == Some(setter)),
        "`setCount(1);`: a written built-in leaves it uncertifiable, found nothing at {setter}"
    );
    let ablated = check(&typefacts, name, Some(LIBRARY_RUNS_NOTHING));
    only_these_moved(&named, &ablated, &BTreeSet::new());
}

#[test]
fn the_owner_reason_ends_exactly_the_calls_with_an_owner_captured_before_the_suspension() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let name = "after-await-universe";
    let source = source(name);
    let named = check(&typefacts, name, None);
    let rows = unaccounted(&named);
    // R5: `runWithOwner(owner, …)` with the owner an unreassigned `const` of `getOwner()` written in the same function ahead of every suspension of it: no row, through the
    // namespace import and the alias too. `untrack` and `createRoot` are no sites.
    let captured = BTreeSet::from([
        at(&source, "runWithOwner(captured, () => count());", ""),
        at(&source, "solid.runWithOwner(owner, () => count());", ""),
        at(&source, "withOwner(owner, () => count());", ""),
    ]);
    for start in captured.iter().chain(&BTreeSet::from([
        at(&source, "untrack(() => count());", ""),
        at(&source, "createRoot(() => count());", ""),
    ])) {
        assert!(!rows.contains(start), "no row is expected at {start}");
    }
    // The mutation test: dropping `owner-captured` files exactly those three. The library reason ends nothing in this program (it defines a property reflectively).
    let ablated = check(&typefacts, name, Some(OWNER_CAPTURED));
    only_these_moved(&named, &ablated, &captured);
    let ablated = check(&typefacts, name, Some(LIBRARY_RUNS_NOTHING));
    only_these_moved(&named, &ablated, &BTreeSet::new());
}

/// The start bytes of every SC1002 uncertifiable row, whatever its blocker (`function-run-unplaced` is the net's, `read-after-await-unaccounted` the accounting's).
fn sc1002_rows(findings: &[serde_json::Value]) -> BTreeSet<u64> {
    findings
        .iter()
        .filter(|finding| finding["id"] == "SC1002")
        .map(|finding| {
            assert_eq!(finding["kind"], "uncertifiable", "never a violation");
            finding["primaryLocation"]["startByte"].as_u64().unwrap()
        })
        .collect()
}

/// The rows of the fixture are exactly the places `anchors` name (an `(anchor, skip)` pair is where `skip` ends inside the first `anchor`).
fn assert_rows(name: &str, anchors: &[(&str, &str)]) {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let source = source(name);
    let rows = sc1002_rows(&check(&typefacts, name, None));
    let expected = anchors
        .iter()
        .map(|(anchor, skip)| at(&source, anchor, skip))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        rows, expected,
        "{name}: the rows are exactly the sites named"
    );
}

#[test]
fn code_a_continuation_runs_with_no_call_written_there_is_a_row() {
    // S1 and R6 of the reviews of #233 (ADR 0976), over a start no producer follows: the reads inside the members the continuation runs without a call (a getter, a `toString`, an
    // iterator method, a constructor, a tag function, a Proxy's `get` and `has` trap: the member-route net's rows) and the member reads, spreads and loops of the full S4 rule
    // (a stored generator object spread, iterated or destructured: its body is placed by a call that only made the object). Every one is uncertifiable, none a violation.
    assert_rows(
        "after-await-net",
        &[
            ("get value() { return source(); }", "get value() { return "),
            ("String(source())", "String("),
            ("yield source(); }", "yield "),
            ("this.v = source();", "this.v = "),
            ("parts.length + source();", "parts.length + "),
            ("get: () => source()", "get: () => "),
            ("has: () => source()", "has: () => "),
            ("return holder.value;", "return "),
            ("const { value } = holder;", "const "),
            ("return { ...holder }.value;", "return "),
            ("{ ...holder }.value", "{ ..."),
            ("of iterable) total", "of "),
            ("return new Reads().v;", "return "),
            ("return proxy.x;", "return "),
            ("[...stored][0]", "[..."),
            ("of storedLoop) return", "of "),
            ("const [value] = storedPattern;", "const "),
        ],
    );
}

#[test]
fn a_promise_handler_is_code_written_after_a_suspension_of_the_function_that_holds_the_chain() {
    // S3 (ADR 0976): `then`, `catch`, `finally` and a handler handed by reference, in functions that are no `async` function; the chain with a computed `["then"]` is an SC9012 at
    // the call; `viaPure` (a handler that reads nothing and calls no project function) lists nothing; `standalone` (no computation starts it) keeps its row: there is no census reason.
    assert_rows(
        "after-await-then-handler",
        &[
            ("return source();\n}\nfunction track", "return "),
            (
                "pause().then(() => source());\n}\nfunction viaCatch",
                "pause().then(() => ",
            ),
            ("catch(() => source())", "catch(() => "),
            ("finally(() => { source(); })", "finally(() => { "),
            (
                "pause().then(() => source());\n}\n\ncreateRoot",
                "pause().then(() => ",
            ),
        ],
    );
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let source = source("after-await-then-handler");
    let computed = at(&source, "pause()[\"then\"](() => source());", "");
    assert!(
        check(&typefacts, "after-await-then-handler", None)
            .iter()
            .any(|finding| finding["id"] == "SC9012"
                && finding["primaryLocation"]["startByte"].as_u64() == Some(computed)),
        "a computed `then` is a handed member dispatch the pass cannot resolve: an SC9012 at {computed}"
    );
}

#[test]
fn a_member_read_of_a_value_not_proven_plain_after_an_await_is_a_row() {
    // S4 (ADR 0976), the full rule: a store or props value reached through a binding the universe does not list (a module-scope alias, a destructuring,
    // an element of a store array, a function's return, a cell of a literal, an alias made by an assignment, an alias made before the `await` in a recognized computation, a props
    // alias), and the objects whose origin no fact proves: a parameter, a spread of a project literal (not proven plain: open-work). `viaPrimitive` (the read is before the `await`
    // and what follows is arithmetic on a number) lists nothing.
    assert_rows(
        "after-await-store-alias",
        &[
            ("return inner.x;", "return "),
            ("return n.x;", "return "),
            ("return first.x;", "return "),
            ("return got.x;", "return "),
            ("return cell.v.x;", "return "),
            ("{ ...plain };", "{ ..."),
            (
                "const local = { ...plain };\n  return local.x;",
                "const local = { ...plain };\n  return ",
            ),
            ("return value.x;", "return "),
            (
                "local = store.n;\n  await pause();\n  return local.x;",
                "local = store.n;\n  await pause();\n  return ",
            ),
            (
                "const local = store.n;\n    await pause();\n    return local.x;",
                "const local = store.n;\n    await pause();\n    return ",
            ),
            ("return alias.y;", "return "),
        ],
    );
}

#[test]
fn a_generator_body_is_a_member_route_of_the_read_after_await_net_only() {
    // ADR 0976, R6: the stepped generator is a member route for SC1002's net. The other rules' member nets (SC3001's leaf walk here) keep their rows: the first build widened
    // `runs_implicitly` for every rule, and 15 `leaf-operation-unaccounted` rows of this fixture turned into `function-run-unplaced` ones (and two were added).
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let findings = check(&typefacts, "rc9-flush-in-action", None);
    let blockers = |id: &str| {
        findings
            .iter()
            .filter(|finding| finding["id"] == id)
            .map(|finding| finding["analysisContext"].as_str().unwrap_or("").to_owned())
            .collect::<BTreeSet<_>>()
    };
    assert!(
        !blockers("SC3001").contains("function-run-unplaced"),
        "SC3001's rows stand on the leaf walk, not on the generator route"
    );
    assert!(
        blockers("SC1002").contains("unresolved-invocation"),
        "the read-after-await net asks the generator body"
    );
}

/// Where `anchor` starts after the first `marker` in `source`.
fn after(source: &str, marker: &str, anchor: &str) -> u64 {
    let from = source
        .find(marker)
        .unwrap_or_else(|| panic!("no `{marker}`"));
    (from + source[from..].find(anchor).unwrap_or_else(|| panic!("no `{anchor}` after `{marker}`"))) as u64
}

/// ADR 0984: a `let` timer handle is an inert argument when every reference of it is a proven write (a plain `=` of a call of the unpatched `setTimeout` or `setInterval`, or
/// `undefined`) or the sole argument of a `clearTimeout` or `clearInterval`, in a browser: its `clearTimeout` ends in no row. A write by a call nothing resolves, a copy, a handing
/// on, a destructured write, a member callee and a `const` copy keep the row; so do the same shape in a program that writes a built-in and under the Node runtime target.
#[test]
fn a_let_timer_handle_is_inert_in_a_browser_when_every_reference_is_a_proven_write_or_a_clear() {
    let typefacts = env::var("SOLID_TYPEFACTS_BIN").expect("arm the real Type Facts process");
    let name = "after-await-timer-handle";
    let source = source(name);
    let rows = unaccounted(&check(&typefacts, name, None));
    for (marker, anchor, what) in [
        ("export function Cleared()", "clearTimeout(timer);\n    timer", "the handle cleared in a promise handler"),
        ("export async function ClearedInterval()", "clearInterval(ticker);", "an interval cleared after the suspension"),
    ] {
        let start = after(&source, marker, anchor);
        assert!(!rows.contains(&start), "{what}: a row at {start}");
    }
    for (marker, anchor, what) in [
        ("export function UnknownWrite()", "clearTimeout(handle);", "a write by a call nothing resolves"),
        ("export function Copied()", "clearTimeout(timer);", "the handle copied into an object"),
        ("export function Handed()", "clearTimeout(timer);", "the handle handed to a function"),
        ("export function Compound()", "clearTimeout(timer);", "a destructured write"),
        ("export function Member()", "clearTimeout(timer);", "a member callee"),
        ("export function ConstCopy()", "clearTimeout(copy);", "a `const` copy of the handle"),
    ] {
        let start = after(&source, marker, anchor);
        assert!(rows.contains(&start), "{what}: no row at {start}");
    }
    // The twins of `Cleared`: a program that writes a built-in, and the Node runtime target.
    for name in ["after-await-timer-handle-patched", "after-await-timer-handle-node"] {
        let twin = self::source(name);
        let rows = unaccounted(&check(&typefacts, name, None));
        let start = after(&twin, "export function Cleared()", "clearTimeout(timer);");
        assert!(rows.contains(&start), "{name}: the same shape keeps its row, none at {start}");
    }
}
