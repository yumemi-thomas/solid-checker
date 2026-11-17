use super::*;

fn unconditional(source: &str, name: &str) -> Vec<String> {
    let mut start = source
        .find(&format!("function {name}"))
        .expect("the function");
    if source[..start].ends_with("async ") {
        start -= "async ".len();
    }
    let end = source[start..]
        .find("\n}")
        .map(|offset| start + offset + 2)
        .expect("the function's end");
    let span = Span::new(start as u32, end as u32);
    unconditional_calls(Path::new("test.js"), source, span, &[])
        .expect("parses")
        .into_iter()
        .map(|span| source[span.start as usize..span.end as usize].to_owned())
        .collect()
}

fn synchronous(source: &str, name: &str) -> Vec<String> {
    let start = source
        .find(&format!("function {name}"))
        .expect("the function");
    let start = if source[..start].ends_with("async ") {
        start - "async ".len()
    } else {
        start
    };
    let end = source[start..]
        .find("\n}")
        .map(|offset| start + offset + 2)
        .expect("the end");
    synchronously_unconditional_calls(
        Path::new("test.js"),
        source,
        Span::new(start as u32, end as u32),
        &[],
    )
    .expect("parses")
    .into_iter()
    .map(|span| source[span.start as usize..span.end as usize].to_owned())
    .collect()
}

/// ADR 0275: an `async` function returns at its own first suspension.
#[test]
fn an_async_function_states_its_calls_before_its_first_suspension() {
    let source = "async function before(cb) {\n  onCleanup(cb);\n  await load();\n  onCleanup(cb);\n}\n\
async function inside(cb) {\n  onCleanup(await load(cb));\n}\n\
async function looped(cb, list) {\n  for await (const x of list) {}\n  onCleanup(cb);\n}\n\
async function nested(cb) {\n  const later = async () => { await load(); };\n  onCleanup(cb);\n}\n\
async function *generated(cb) {\n  onCleanup(cb);\n}\n\
function plain(cb) {\n  onCleanup(cb);\n}\n\
function guarded(cb, ready) {\n  if (!ready) throw new Error('x');\n  onCleanup(cb);\n}\n";
    assert_eq!(synchronous(source, "before"), ["onCleanup(cb)"]);
    assert!(synchronous(source, "inside").is_empty());
    assert!(synchronous(source, "looped").is_empty());
    assert_eq!(synchronous(source, "nested"), ["onCleanup(cb)"]);
    assert!(synchronous(source, "*generated").is_empty());
    assert_eq!(synchronous(source, "plain"), ["onCleanup(cb)"]);
    assert!(unconditional(source, "before").is_empty());
    // A caller may catch the thrown guard and carry on (ADR 0276).
    assert!(synchronous(source, "guarded").is_empty());
    assert_eq!(unconditional(source, "guarded"), ["onCleanup(cb)"]);
}

#[test]
fn only_a_call_on_every_completion_is_unconditional() {
    let source = "function straight(cb) {\n  onCleanup(cb);\n  const x = createEffect(a, b);\n}\n\
function guarded(cb, flag) {\n  if (flag) onCleanup(cb);\n  flag && onCleanup(cb);\n  flag ? onCleanup(cb) : 0;\n  cb?.();\n}\n\
function early(cb, flag) {\n  if (flag) return;\n  onCleanup(cb);\n}\n\
function thrown(cb, flag) {\n  if (!flag) throw new Error('x');\n  onCleanup(cb);\n}\n\
function looped(cb) {\n  for (;;) onCleanup(cb);\n  try { onCleanup(cb); } catch {}\n}\n\
function nested(cb) {\n  queueMicrotask(() => onCleanup(cb));\n}\n\
async function later(cb) {\n  onCleanup(cb);\n}\n";
    assert_eq!(
        unconditional(source, "straight"),
        ["onCleanup(cb)", "createEffect(a, b)"]
    );
    assert!(unconditional(source, "guarded").is_empty());
    assert!(unconditional(source, "early").is_empty());
    assert_eq!(unconditional(source, "thrown"), ["onCleanup(cb)"]);
    assert!(unconditional(source, "looped").is_empty());
    assert_eq!(
        unconditional(source, "nested"),
        ["queueMicrotask(() => onCleanup(cb))"]
    );
    assert!(unconditional(source, "later").is_empty());
}

/// The function bodies of two published exports, byte for byte:
/// `@solid-primitives/gestures@3.0.0-next.3` `dist/tap.js` (sha256
/// 270f7a35...9174) and `@solid-primitives/mutation-observer@3.0.0-next.2`
/// `dist/index.js` (sha256 8d970f9c...8f20). `tap` registers its
/// `onCleanup` on every call; `createMutationObserver` registers
/// `onSettled` and `onCleanup` only `if (isSupported)`.
#[test]
fn a_published_export_registers_on_every_call_only_without_a_guard() {
    const TAP: &str = r#"function tap(props) {
	let cleanup;
	onCleanup(() => cleanup?.());
	return (node) => {
		let x;
		let y;
		let time;
		let startPointerId = null;
		const downCallback = (activeEvents, event) => {
			if (activeEvents.length !== 1) return;
			startPointerId = event.pointerId;
			time = Date.now();
			x = event.clientX;
			y = event.clientY;
		};
		const upCallback = (_, event) => {
			if (event.pointerId !== startPointerId) return;
			startPointerId = null;
			const now = Date.now();
			if (Math.abs(event.clientX - x) < 4 && Math.abs(event.clientY - y) < 4 && now - time >= (props.minimumTapLength ?? 0) && (props.maximumTapLength === void 0 || now - time < props.maximumTapLength)) {
				const rect = node.getBoundingClientRect();
				const tapX = Math.round(event.clientX - rect.left);
				const tapY = Math.round(event.clientY - rect.top);
				props.callback({
					x: tapX,
					y: tapY
				});
			}
		};
		cleanup = registerPointerListener(node, downCallback, void 0, upCallback);
	};
}
"#;
    const MUTATION_OBSERVER: &str = r#"function createMutationObserver(initial, b, c) {
	let defaultOptions, callback;
	const isSupported = !isServer;
	if (typeof b === "function") {
		defaultOptions = {};
		callback = b;
	} else {
		defaultOptions = b;
		callback = c;
	}
	const instance = isSupported ? new MutationObserver(callback) : void 0;
	const add = (el, options) => instance?.observe(el, access(options) ?? defaultOptions);
	const start = () => {
		if (!isSupported) return;
		asArray(access(initial)).forEach((item) => {
			item instanceof Node ? add(item, defaultOptions) : add(item[0], item[1]);
		});
	};
	const stop = () => instance?.disconnect();
	if (isSupported) {
		onSettled(start);
		onCleanup(stop);
	}
	return [add, {
		start,
		stop,
		instance,
		isSupported
	}];
}
"#;
    let tap = unconditional(TAP, "tap");
    assert!(
        tap.iter()
            .any(|call| call == "onCleanup(() => cleanup?.())")
    );
    let observer = unconditional(MUTATION_OBSERVER, "createMutationObserver");
    assert!(!observer.iter().any(|call| call == "onSettled(start)"));
    assert!(!observer.iter().any(|call| call == "onCleanup(stop)"));
}

/// ADR 0166: under a declared host, `isServer` is the value the host's
/// resolution of `@solidjs/web` binds (`false` in `dist/web.js`, `true` in
/// `dist/server.js`). The bodies are byte for byte
/// `@solid-primitives/memo@2.0.0-next.2` `dist/index.js` `createPureReaction`
/// and `@solid-primitives/lifecycle@1.0.0-next.2` `dist/index.js`
/// `onElementConnect`, under the import line both modules write.
#[test]
fn a_folded_host_constant_decides_the_guard() {
    const MEMO: &str = r#"import { isServer } from "@solidjs/web";
function createPureReaction(onInvalidate, options) {
	if (isServer) return () => void 0;
	const owner = getOwner();
	let trackers = 0;
	let disposed = false;
	onCleanup(() => {
		disposed = true;
	});
	return (tracking) => {
		if (disposed) {
			untrack(tracking);
			return;
		}
		trackers++;
		runWithOwner(owner, () => createReaction(() => {
			if (--trackers === 0) untrack(onInvalidate);
		}, options))(tracking);
	};
}"#;
    const LIFECYCLE: &str = r#"import { isServer } from "@solidjs/web";
function onElementConnect(el, fn) {
	if (isServer) return;
	if (el.isConnected) return fn();
	const observer = new ResizeObserver(() => el.isConnected && (observer.disconnect(), fn()));
	observer.observe(el);
	onCleanup(() => observer.disconnect());
}"#;
    let fold = |value| {
        vec![HostConstantFold {
            specifier: "@solidjs/web".into(),
            imported: "isServer".into(),
            value,
        }]
    };
    let calls = |source: &str, name: &str, folds: &[HostConstantFold]| {
        let start = source
            .find(&format!("function {name}"))
            .expect("the function");
        let end = source[start..]
            .find("\n}")
            .map(|offset| start + offset + 2)
            .expect("end");
        unconditional_calls(
            Path::new("index.js"),
            source,
            Span::new(start as u32, end as u32),
            folds,
        )
        .expect("parses")
        .into_iter()
        .map(|span| source[span.start as usize..span.end as usize].to_owned())
        .collect::<Vec<_>>()
    };
    let cleanup = |calls: &[String]| calls.iter().any(|call| call.starts_with("onCleanup("));
    // Host free: the guard may return, so nothing after it is unconditional.
    assert!(!cleanup(&calls(MEMO, "createPureReaction", &[])));
    // Browser: the guard never returns.
    let browser = calls(MEMO, "createPureReaction", &fold(false));
    assert!(cleanup(&browser), "{browser:?}");
    assert!(browser.iter().any(|call| call == "getOwner()"));
    // Node: the guard always returns, so nothing after it runs at all.
    assert!(calls(MEMO, "createPureReaction", &fold(true)).is_empty());
    // `onElementConnect` also returns under `el.isConnected`, which no fold
    // decides: its `onCleanup` stays conditional under every host.
    for folds in [Vec::new(), fold(false), fold(true)] {
        assert!(!cleanup(&calls(LIFECYCLE, "onElementConnect", &folds)));
    }
}

/// ADR 0550: a generator's first step runs its calls before its first
/// `yield` (or, async, its first `await`); calling a plain function asks
/// nothing of this census.
#[test]
fn a_generator_first_step_states_its_calls_before_its_first_yield() {
    let source = "function* plain(cb) {\n  createMemo(cb);\n  if (cb) onCleanup(cb);\n  yield 1;\n  onCleanup(cb);\n}\n\
async function* awaited(cb) {\n  createMemo(cb);\n  await load();\n  onCleanup(cb);\n}\n\
function* operand(cb) {\n  yield createMemo(cb);\n}\n\
function notGenerator(cb) {\n  createMemo(cb);\n}\n";
    let first_step = |name: &str| {
        let start = source.find(&format!("{name}(cb)")).expect("the function");
        let start = source[..start].rfind("function").expect("the keyword");
        let start = if source[..start].ends_with("async ") {
            start - "async ".len()
        } else {
            start
        };
        let end = source[start..]
            .find("\n}")
            .map(|offset| start + offset + 2)
            .expect("the end");
        first_step_unconditional_calls(
            Path::new("test.js"),
            source,
            Span::new(start as u32, end as u32),
            &[],
        )
        .map(|spans| {
            spans
                .into_iter()
                .map(|span| source[span.start as usize..span.end as usize].to_owned())
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(first_step("plain"), Some(vec!["createMemo(cb)".to_owned()]));
    assert_eq!(
        first_step("awaited"),
        Some(vec!["createMemo(cb)".to_owned()])
    );
    assert_eq!(first_step("operand"), Some(Vec::new()));
    assert_eq!(first_step("notGenerator"), None);
}

fn every(source: &str) -> Vec<String> {
    let mut calls = every_unconditional_call(Path::new("test.tsx"), source)
        .expect("parses")
        .into_iter()
        .collect::<Vec<_>>();
    calls.sort_by_key(|span| span.start);
    calls
        .into_iter()
        .map(|span| source[span.start as usize..span.end as usize].to_owned())
        .collect()
}

/// ADR 0714: module scope is a scope of its own; a branch, a `&&` operand
/// or a call after an earlier `throw` is not admitted there.
#[test]
fn every_unconditional_call_admits_module_scope_statements() {
    let source = "const x = a();\nexport const s = { args: { v: b() } };\nexport default c();\n\
                  if (Math.random() > 2) d();\nq && e();\n(() => f())();\n";
    assert_eq!(
        every(source),
        ["a()", "b()", "c()", "Math.random()", "(() => f())()", "f()"]
    );
    assert_eq!(every("g();\nthrow 1;\nh();\n"), ["g()"]);
    assert_eq!(
        every("a();\nawait p;\nb(await c());\n"),
        ["a()", "b(await c())", "c()"]
    );
    assert_eq!(
        every(
            "class K { static s = a(); t = b(); static [c()] = 1; }\nif (q) { class L { static u = d(); } }\n"
        ),
        ["a()"]
    );
}

/// ADR 0714: a function's calls are its own scope's: a nested function's
/// calls answer for that function, not the call around it.
#[test]
fn every_unconditional_call_answers_per_function() {
    let source = "function k(x) {\n  a();\n  if (x) b();\n  return () => c();\n}\n\
                  async function l() { d(); await p; e(); }\n\
                  function* m() { f(); yield 1; g(); }\n";
    assert_eq!(every(source), ["a()", "c()", "d()", "e()", "f()"]);
}

/// ADR 1042: under a label, in a finalizer or a `try` block no `catch` guards, in a `do` body, and in the body of a loop
/// whose first test is true, a call runs at least once on every completion; every counterexample of the brief stays out.
#[test]
fn at_least_once_admits_labels_finalizers_do_bodies_and_entered_loops() {
    for (source, admitted) in [
        (
            "function f() {\n  outer: { a(); break outer; }\n}\n",
            vec!["a()"],
        ),
        (
            "function f() {\n  try {} finally { a(); }\n}\n",
            vec!["a()"],
        ),
        (
            "function f() {\n  try { a(); } finally {}\n}\n",
            vec!["a()"],
        ),
        (
            "function f() {\n  do { a(); } while (false);\n}\n",
            vec!["a()"],
        ),
        (
            "function f() {\n  while (true) { a(); break; }\n}\n",
            vec!["a()"],
        ),
        (
            "function f() {\n  for (;;) { a(); break; }\n}\n",
            vec!["a()"],
        ),
        (
            "function f() {\n  for (h(); i(); ) { a(); break; }\n}\n",
            vec!["h()", "i()"],
        ),
        (
            "function f() {\n  for (const x of k()) {}\n}\n",
            vec!["k()"],
        ),
        ("function f() {\n  while (true) a();\n}\n", vec!["a()"]),
        (
            "function f() {\n  outer: while (true) { a(); break outer; }\n}\n",
            vec!["a()"],
        ),
        (
            "function f() {\n  try { n(); } catch {} finally { a(); }\n}\n",
            vec!["a()"],
        ),
        (
            "function f(c) {\n  try { return c; } finally { a(); }\n}\n",
            vec!["a()"],
        ),
    ] {
        assert_eq!(synchronous(source, "f"), admitted, "{source}");
        // No exactly-once call: `unconditional_calls` and the producer's whitelist are unchanged.
        assert!(unconditional(source, "f").is_empty(), "{source}");
    }
    for (source, kept) in [
        (
            "function f(c) {\n  outer: { if (c) break outer; a(); }\n}\n",
            "a()",
        ),
        ("function f(c) {\n  try { x(); } catch { a(); }\n}\n", "a()"),
        (
            "function f(c) {\n  try {} finally { if (c) return; a(); }\n}\n",
            "a()",
        ),
        ("function f(c) {\n  try { x(); a(); } catch {}\n}\n", "a()"),
        (
            "function f(c, x) {\n  do { if (c) break; a(); } while (x);\n}\n",
            "a()",
        ),
        ("function f(c) {\n  do {} while (a());\n}\n", "a()"),
        (
            "function f(n) {\n  for (let i = 0; i < n; i++) a();\n}\n",
            "a()",
        ),
        ("function f(xs) {\n  for (const x of xs) a();\n}\n", "a()"),
        ("function f(c) {\n  while (c) a();\n}\n", "a()"),
        (
            "function f(c) {\n  while (true) { if (c) continue; a(); break; }\n}\n",
            "a()",
        ),
        (
            "function f() {\n  for (let i = 0; i < 0; i++) a();\n}\n",
            "a()",
        ),
        ("function f() {\n  for (;; a()) { break; }\n}\n", "a()"),
        (
            "function f(c) {\n  while (true) { if (c) break; }\n  a();\n}\n",
            "a()",
        ),
    ] {
        assert!(
            !synchronous(source, "f").contains(&kept.to_owned()),
            "{source}"
        );
    }
}

/// ADR 1042: the owner pass's set (`every_unconditional_call`) reads the same mode, a nested function answering for itself.
#[test]
fn every_unconditional_call_reads_at_least_once() {
    let source = "function k(c) {\n  outer: { a(); }\n  try {} finally { b(); }\n  while (c) e();\n}\n\
                  function l() {\n  while (true) { const g = () => c1(); d(); break; }\n}\n";
    assert_eq!(every(source), ["a()", "b()", "c1()", "d()"]);
}

/// ADR 1046: a counter loop whose literal bounds pass the first test, and the case a literal `switch` enters, run at least
/// once; zero-iteration bounds, an early exit, a non-literal test, fall-through and `default` stay out.
#[test]
fn at_least_once_admits_counter_loops_and_literal_switch_cases() {
    for (source, admitted) in [
        (
            "function f() {\n  for (let i = 0; i < 1; i += 1) { a(); }\n}\n",
            true,
        ),
        (
            "function f() {\n  for (let i = 0; i < 3; i++) a();\n}\n",
            true,
        ),
        (
            "function f() {\n  for (var i = 5; i >= 0; i--) { a(); }\n}\n",
            true,
        ),
        (
            "function f() {\n  for (let i = 0; i !== 2; i++) { a(); }\n}\n",
            true,
        ),
        (
            "function f() {\n  switch ((1)) {\n    case 1: { a(); break; }\n  }\n}\n",
            true,
        ),
        (
            "function f() {\n  switch (\"b\") {\n    case \"a\": x(); break;\n    case \"b\": a(); break;\n  }\n}\n",
            true,
        ),
        (
            "function f() {\n  switch (true) {\n    case false: break;\n    case true: a();\n  }\n}\n",
            true,
        ),
        // Zero iterations, an exit before the call, a test no literal decides.
        (
            "function f() {\n  for (let i = 0; i < 0; i++) a();\n}\n",
            false,
        ),
        (
            "function f() {\n  for (let i = 1; i <= 0; i++) a();\n}\n",
            false,
        ),
        (
            "function f(n) {\n  for (let i = 0; i < n; i++) a();\n}\n",
            false,
        ),
        (
            "function f(c) {\n  for (let i = 0; i < 1; i++) { if (c) break; a(); }\n}\n",
            false,
        ),
        (
            "function f(c) {\n  for (let i = 0; i < 1; i++) { if (c) continue; a(); }\n}\n",
            false,
        ),
        (
            "function f(j) {\n  for (let i = 0; j < 1; i++) a();\n}\n",
            false,
        ),
        (
            "function f() {\n  for (let i = 0, j = 1; i < 1; i++) a();\n}\n",
            false,
        ),
        // A case no literal decides, a missed case, an earlier equal case, fall-through, `default`, an early `break`.
        (
            "function f(x) {\n  switch (x) {\n    case 1: a();\n  }\n}\n",
            false,
        ),
        (
            "function f() {\n  switch (1) {\n    case g(): break;\n    case 1: a();\n  }\n}\n",
            false,
        ),
        (
            "function f() {\n  switch (2) {\n    case 1: a();\n  }\n}\n",
            false,
        ),
        (
            "function f() {\n  switch (1) {\n    case 1: break;\n    case 1: a();\n  }\n}\n",
            false,
        ),
        (
            "function f() {\n  switch (1) {\n    case 1: x();\n    case 2: a();\n  }\n}\n",
            false,
        ),
        (
            "function f() {\n  switch (1) {\n    default: a();\n  }\n}\n",
            false,
        ),
        (
            "function f(c) {\n  switch (1) {\n    case 1: if (c) break; a();\n  }\n}\n",
            false,
        ),
        (
            "function f() {\n  switch (1) {\n    case \"1\": a();\n  }\n}\n",
            false,
        ),
    ] {
        assert_eq!(
            synchronous(source, "f").contains(&"a()".to_owned()),
            admitted,
            "{source}"
        );
        assert!(
            !unconditional(source, "f").contains(&"a()".to_owned()),
            "{source}"
        );
    }
}
