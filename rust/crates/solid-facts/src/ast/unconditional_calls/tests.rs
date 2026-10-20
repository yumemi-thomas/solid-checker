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
