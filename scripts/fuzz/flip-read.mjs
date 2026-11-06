// The flip's read track (scripts/flip/PLAN.md): recall contexts per runtime code, and the program of each. Empty at the base: a rule's
// commit adds its shapes here, so neither track edits misuse.mjs or the other's file.
//   FLIP_READ_CONTEXTS  { [code]: [context, ...] }, appended to that code's recallContexts
//   flipReadContext(context, stmts, kit)  the program, as a misuse.mjs context returns one, or undefined when the context is not this file's
//   FLIP_READ_LEGAL     { [code]: [context, ...] }, appended to that code's recallLegal
//   FLIP_READ_VARIANTS  { [code]: { [variant]: { kind, contexts } } }, a source of that kind, in those contexts, added to the code's recall variants
import { apiSignal, button, call, child, decl, domExpr, expr, fn, iife, jsxExpr, mounted, num, prim, ref, ret } from "./model.mjs";

// ADR 0654: `dynamic(source, options)` as each variant writes it: every one throws in dev, by the source's shape or the options'.
export const STATIC_SOURCES = {
  "src-block": '() => { return Promise.resolve("p" as const); }, { static: true }',
  "src-then": '() => Promise.resolve(1).then(() => "p" as const), { static: true }',
  "src-any": '(() => Promise.resolve("p" as const)) as () => any, { static: true }',
  "src-cond": '() => (Math.random() >= 0 ? Promise.resolve("p" as const) : ("p" as const)), { static: true }',
  "src-options": '() => Promise.resolve("p" as const), ({ static: true }) as { static: boolean }',
};

// ADR 0655: a `sync: true` computation whose return no type settles. Each returns a Promise when it runs.
export const SYNC_COMPUTES = {
  "cmp-any": '((() => Promise.resolve(1)) as () => any), { sync: true }',
  "cmp-unknown": '((() => Promise.resolve(1)) as () => unknown), { sync: true }',
  "cmp-cond": '() => (Math.random() >= 0 ? Promise.resolve(1) : 1), { sync: true }',
  "cmp-options": 'async () => 1, ({ sync: true }) as { sync: boolean }',
};

// ADR 0657: a destructure no exact root reaches. Each is a Get of the store at setup.
export const STORE_DESTRUCTURES = {
  "store-dstr-assign": { reads: true, text: (o) => `let v = 0; ({ v } = ${o}); void v;` },
  "store-dstr-array": { reads: true, text: (o) => `const [{ v }] = [${o}]; void v;` },
  "store-dstr-for-of": { reads: true, text: (o) => `for (const { v } of [${o}]) void v;` },
  "store-dstr-merge": { reads: true, imports: ["merge"], text: (o) => `const { v } = merge(${o}, { w: 1 }); void v;` },
};

// ADR 0781: the type names a standard method, and the program runs its own. Each runs the site on the caller's stack, in a component body.
const SYNC_THEN = "{ then(f: () => void) { f(); } }";
const PATCHES = {
  "patch-global": ["(globalThis as any).setTimeout = (f: () => void) => { f(); return 0; };", "setTimeout"],
  "patch-window": ["window.setTimeout = ((f: () => void) => { f(); return 0; }) as any;", "setTimeout"],
  "patch-define": ['Object.defineProperty(globalThis, "setTimeout", { value: (f: () => void) => { f(); return 0; } });', "setTimeout"],
  "patch-proto": ["Promise.prototype.then = function (this: any, f: any) { f(); return this; } as any;", "Promise.resolve().then"],
  "patch-proto-alias": ["const proto = Promise.prototype; proto.then = function (this: any, f: any) { f(); return this; } as any;", "Promise.resolve().then"],
};
const CASTS = {
  "cast-then": (t) => ({ held: [decl(t, domExpr(SYNC_THEN))], callee: `(${t} as unknown as Promise<void>).then` }),
  "cast-any-then": (t) => ({ held: [decl(t, domExpr(`${SYNC_THEN} as any`))], callee: `(${t} as Promise<void>).then` }),
  "cast-held-then": (t, q) => ({ held: [decl(t, domExpr(SYNC_THEN)), decl(q, domExpr(`${t} as unknown as Promise<void>`))], callee: `${q}.then` }),
};
const LYING = [...Object.keys(CASTS), ...Object.keys(PATCHES)];

export const FLIP_READ_CONTEXTS = {
  LOADING_ON_ACCESSOR: ["on-any", "on-unknown", "on-spread", "on-json", "on-prop", "on-call"],
  // ADR 0662, ADR 0668: a `<Dynamic>` body has no strict window and still warns with no boundary; a `<Portal>` is a plain synchronous mount.
  ASYNC_OUTSIDE_LOADING_BOUNDARY: ["dyn-jsx-body", "portal-jsx", "portal-dyn"],
  PENDING_ASYNC_UNTRACKED_READ: ["portal-body", "dyn-jsx-mixed"],
  // ADR 0669: a memo's promise chain with a handler handed by name.
  UNTRACKED_READ_AFTER_AWAIT: ["then-named"],
  // ADR 0679: a named function holding a window-inheriting call, run directly from the body.
  STRICT_READ_UNTRACKED: ["root-named-twice", "root-named-foreach", ...LYING],
  // ADR 0782: a mount outside the cleanup, or in a handler, keeps the guard's throw.
  REACTIVE_WRITE_IN_OWNED_SCOPE: [...LYING, "cleanup-mount-mixed", "handler-mount"],
};

// ADR 0661, ADR 0668: quiet in Chrome. A mount after `render()` returned has no boundary check, and the source settled; a `<Dynamic>` body has
// no window, so a pending read retries. Under `<Loading>`: no placeless boundary warning leaves the program unjudged.
export const FLIP_READ_LEGAL = {
  ASYNC_OUTSIDE_LOADING_BOUNDARY: ["show-false-later", "ternary-later", "and-later", "dyn-jsx-later"],
  PENDING_ASYNC_UNTRACKED_READ: ["dyn-jsx-body-l", "dyn-fn-body-l"],
  // ADR 0678: `render(Local, el)` calls `Local` under a root, with no strict window. ADR 0677: a client build never runs a helper only `isServer` reaches.
  // ADR 0781: `cast-late-foreach` hands the callback to a microtask, whatever the assertion says.
  STRICT_READ_UNTRACKED: ["mount-named", "arm-helper", "arm-helper-chain", "arm-tag", "root-named-timer", "root-named-then", "root-two-handlers", "apply-cleanup-mount", "subclass-late-foreach", "cast-late-foreach"],
  // ADR 0782: `cleanup-mount`'s only mount is in a cleanup, where the guard stands down.
  REACTIVE_WRITE_IN_OWNED_SCOPE: ["arm-helper", "arm-helper-chain", "arm-tag", "cast-late-foreach", "cleanup-mount"],
  STORE_WRITE_OUTSIDE_SETTER: ["arm-helper"],
};

// ADR 0665: a source no type proves sync or async. Each is pending at the read; the build before took it for a settled signal.
// ADR 0689: a hook's result used on the spot, or iterated: the hook's closure reads what the site handed it, and no binding holds the result.
const HANDED_HOOKS = ["body", "effect-apply", "root", "render-apply"];
export const HANDED_READS = {
  "handed-chain": { kind: "plain", contexts: HANDED_HOOKS, text: (o) => `const useT = (n: () => number) => ({ read: (): number => n() }); void useT(${o}).read();` },
  "handed-curried": { kind: "plain", contexts: HANDED_HOOKS, text: (o) => `const useT = (n: () => number) => () => () => n(); void useT(${o})()();` },
  "handed-element": { kind: "plain", contexts: HANDED_HOOKS, text: (o) => `const useT = (n: () => number) => [(): number => n()]; useT(${o}).forEach((f) => void f());` },
  "handed-held-element": { kind: "plain", contexts: HANDED_HOOKS, text: (o) => `const useT = (n: () => number) => [(): number => n()]; const x = useT(${o}); x.forEach((f) => void f());` },
  "handed-element-member": { kind: "plain", contexts: HANDED_HOOKS, text: (o) => `const useT = (n: () => number) => [{ m: (): number => n() }]; const x = useT(${o}); x.forEach((e) => e.m());` },
};
// ADR 0690: what these primitives return is a memo's accessor. Each makes one and reads it, in a strict window or a tracked one.
const CREATED = { kind: "plain", contexts: ["body", "root"], legal: ["memo", "effect-compute"] };
export const CREATED_READS = {
  "created-children": { ...CREATED, imports: ["children"], text: (o) => `const r = children(() => ${o}()); void r();` },
  "created-children-array": { ...CREATED, imports: ["children"], text: (o) => `const r = children(() => ${o}()); void r.toArray();` },
  "created-map-array": { ...CREATED, imports: ["mapArray"], text: (o) => `const r = mapArray(() => [${o}()], (x) => x); void r();` },
  "created-repeat": { ...CREATED, imports: ["repeat"], text: (o) => `const r = repeat(() => ${o}(), (i) => i); void r();` },
  "created-memo-call": { ...CREATED, imports: ["createMemo"], text: (o) => `void createMemo(() => ${o}())();` },
  // ADR 0780: the same from `solid-js/internal`, whose boundaries return their memo too.
  "created-error-boundary": { ...CREATED, imports: ["createErrorBoundary"], text: (o) => `const r = createErrorBoundary(() => ${o}(), () => 0); void r();` },
  "created-loading-boundary": { ...CREATED, imports: ["createLoadingBoundary"], text: (o) => `const r = createLoadingBoundary(() => ${o}(), () => 0); void r();` },
};
// ADR 0780: `export * as Api from "solid-js"` names every primitive, with no `import * as` of Solid in the project.
const STAR_AS = { kind: "plain", importLines: ['import { Api } from "./solid-api";'], files: { "solid-api.ts": 'export * as Api from "solid-js";\n' } };
export const STAR_AS_READS = {
  "star-as-signal": { ...STAR_AS, contexts: ["body", "root"], legal: ["memo", "effect-compute"], text: () => "const [r] = Api.createSignal(1); void r();" },
  "star-as-memo": { ...STAR_AS, contexts: ["body", "root"], legal: ["memo", "effect-compute"], text: () => "const r = Api.createMemo(() => 1); void r();" },
};
export const STAR_AS_WRITES = {
  "star-as-store": { ...STAR_AS, contexts: ["body", "handler"], text: () => "const [r] = Api.createStore({ v: 1 }); r.v = 2;" },
};
// ADR 0696: a prop's function, run only behind a prop of the component. A caller that passes it warns; one that leaves it out runs nothing.
const guarded = (attribute) => (o) => `const K = (p: { r?: () => number; on?: boolean }) => { if (p.on) void p.r!(); return null; }; void children(() => <K ${attribute}r={() => ${o}()} />)();`;
export const ABSENT_GUARD = {
  "guard-passed": { kind: "plain", contexts: ["body"], imports: ["children"], text: guarded("on ") },
  "guard-absent": { kind: "plain", contexts: [], legal: ["memo"], imports: ["children"], text: guarded("") },
};
// ADRs 0770-0772, the handed net (research report section 1). A prop's member past the first step, a value handed to a helper's parameter at a
// tracked position, a hook no call edge resolves. Each writes, acts or reads through a value no fact resolves; the build before certified each.
const SET = "const [, w] = createSignal(0);";
const USE = `const useT = () => { ${SET} return { go: w }; };`;
const HOST = { kind: "plain", imports: ["createSignal"] };
const NESTED = { ...HOST, imports: ["children", "createSignal"], contexts: ["body"] };
const nested = (value) => () => `${SET} const K = (p: { o: { go: (v: number) => void } }) => { p.o.go(1); return null; }; void children(() => <K o={{ go: ${value} }} />)();`;
const PARAM = { ...HOST, contexts: ["memo", "effect-compute"] };
const NOT_RUN = { ...HOST, contexts: [], legal: ["memo", "effect-compute"] };
const run = "const run = (g: (v: number) => void) => { g(1); return 1; };";
const runMember = "const run = (o: { go: (v: number) => void }) => { o.go(1); return 1; };";
const HOOK = { ...HOST, imports: ["createMemo", "createSignal"], contexts: ["body", "memo"] };
const inMemo = "createMemo(() => { t.go(1); return 0; });";
export const NET_WRITES = {
  "net-nested": { ...NESTED, text: nested("w") },
  // Quiet, and no fact at the call says so: uncertifiable, on purpose.
  "net-nested-plain": { ...NESTED, contexts: [], legal: ["memo"], text: nested("() => {}") },
  "net-param": { ...PARAM, text: () => `${run} ${SET} run(w);` },
  "net-param-member": { ...PARAM, text: () => `${runMember} ${SET} run({ go: w });` },
  "net-param-helper": { ...PARAM, text: () => `${runMember} ${SET} const h = () => run({ go: w }); h();` },
  "net-param-plain": { ...NOT_RUN, text: () => `${run} run(() => {});` },
  "net-param-unused": { ...NOT_RUN, text: () => `const keep = (_g: (v: number) => void) => 1; ${SET} keep(w);` },
  "net-param-later": { ...NOT_RUN, text: () => `const later = (g: (v: number) => void) => () => g(1); ${SET} later(w);` },
  "net-hook-rebound": { ...HOOK, text: () => `${USE} let hook = (() => ({ go: (_v: number) => {} })) as typeof useT; if (Math.random() >= 0) hook = useT; const t = hook(); ${inMemo}` },
  "net-hook-computed": { ...HOOK, text: () => `${USE} const hooks = [useT]; const t = hooks[0](); ${inMemo}` },
  "net-hook-call": { ...HOOK, text: () => `${USE} const t = useT.call(null); ${inMemo}` },
  "net-hook-bind": { ...HOOK, text: () => `${USE} const t = useT.bind(null)(); ${inMemo}` },
  "net-hook-chosen": { ...HOOK, text: () => `${USE} const hook = Math.random() >= 0 ? useT : useT; const t = hook(); ${inMemo}` },
};
export const NET_READS = {
  "net-nested-read": { kind: "plain", contexts: ["body"], imports: ["children"], text: (o) => `const K = (p: { o: { go: () => number } }) => { void p.o.go(); return null; }; void children(() => <K o={{ go: ${o} }} />)();` },
  // ADR 0689's chosen hook: closed on the checker before any shape held it.
  "net-hook-chosen-read": { kind: "plain", contexts: ["body", "root"], legal: ["memo"], text: (o) => `const useA = () => ({ read: ${o} }); const hook = Math.random() >= 0 ? useA : useA; void hook().read();` },
};
export const NET_ACTIONS = {
  "net-param-action": { ...HOST, imports: ["action"], contexts: ["memo", "effect-compute"], text: () => `const run = (g: () => unknown) => { g(); return 1; }; const act = action(function* () { yield 1; }); run(act);` },
};
const PEEK = { kind: "async", contexts: ["jsx", "show"] };
export const NET_PENDING = {
  "net-peek": { ...PEEK, text: (o) => `const peek = (g: () => number) => g(); void peek(${o});` },
  "net-peek-member": { ...PEEK, text: (o) => `const peek = (h: { go: () => number }) => h.go(); void peek({ go: ${o} });` },
};
const STORE_CALL = { kind: "store-array", contexts: ["body", "handler"] };
export const NET_STORE_CALLS = {
  "net-push-bind": { ...STORE_CALL, text: (o) => `Array.prototype.push.bind(${o})(1);` },
  "net-push-held": { ...STORE_CALL, text: (o) => `const f = Array.prototype.push.bind(${o}); f(1);` },
  "net-push-reflect": { ...STORE_CALL, text: (o) => `Reflect.apply(Array.prototype.push, ${o}, [1]);` },
  "net-push-apply-held": { ...STORE_CALL, text: (o) => `const more = [1]; Array.prototype.push.apply(${o}, more);` },
};

export const FLIP_READ_VARIANTS = {
  PENDING_ASYNC_UNTRACKED_READ: { "open-any": { kind: "open-any", contexts: ["body", "root"] }, "open-notready": { kind: "open-notready", contexts: ["body", "root"] } },
  STRICT_READ_UNTRACKED: { ...HANDED_READS, ...CREATED_READS, ...ABSENT_GUARD, ...NET_READS, ...STAR_AS_READS },
  REACTIVE_WRITE_IN_OWNED_SCOPE: NET_WRITES,
  ACTION_CALLED_IN_OWNED_SCOPE: NET_ACTIONS,
  ASYNC_OUTSIDE_LOADING_BOUNDARY: NET_PENDING,
  STORE_WRITE_OUTSIDE_SETTER: { ...NET_STORE_CALLS, ...STAR_AS_WRITES },
};
export const openSignal = (name, kind) => apiSignal(name, 0, kind, "src");

// A value of the boundary's own, beside the site: the context is the way `on` is handed. The boundary is built in the body: a view child must be a number.
const loadingOnForms = (t) => ({
  "on-any": { body: [decl("f", domExpr(`${t} as any`))], tag: "<Loading on={f} fallback={0}><i /></Loading>" },
  "on-unknown": { body: [decl("f", domExpr(`${t} as unknown`))], tag: "<Loading on={f} fallback={0}><i /></Loading>" },
  "on-spread": { body: [decl("p", domExpr(`{ on: ${t} }`))], tag: "<Loading {...p} fallback={0}><i /></Loading>" },
  "on-json": { body: [decl("o", domExpr(`JSON.parse("{}")`))], tag: "<Loading on={o.f} fallback={0}><i /></Loading>", first: `o.f = ${t};` },
  "on-prop": { body: [decl("B", domExpr("(props: { on: unknown }) => <Loading on={props.on} fallback={0}><i /></Loading>"))], tag: `<B on={${t}} />` },
  "on-call": { body: [], tag: `Loading({ on: ${t}, fallback: 0, children: <i /> }) as any` },
});

// The component that holds the site, and how a parent mounts it.
const holderForms = (k) => ({
  "dyn-jsx-body": { read: "body", mount: `<Dynamic component={${k}} />` },
  "dyn-jsx-mixed": { read: "body", mount: `<span><Dynamic component={${k}} /><${k} /></span>` },
  "dyn-jsx-body-l": { read: "body", mount: `<Loading fallback={0}><Dynamic component={${k}} /></Loading>` },
  "dyn-fn-body-l": { read: "body", mount: `<Loading fallback={0}><D${k} /></Loading>`, prelude: `const D${k} = dynamic(() => ${k});` },
  "portal-body": { read: "body", mount: `<Portal><${k} /></Portal>` },
  "portal-jsx": { read: "jsx", mount: `<Portal><${k} /></Portal>` },
  "portal-dyn": { read: "body", mount: `<Portal><Dynamic component={${k}} /></Portal>` },
  "show-false-later": { read: "jsx", later: (on) => `<Show when={${on}[0]()}><${k} /></Show>` },
  "ternary-later": { read: "jsx", later: (on) => `{${on}[0]() ? <${k} /> : null}` },
  "and-later": { read: "jsx", later: (on) => `{${on}[0]() && <${k} />}` },
  "dyn-jsx-later": { read: "jsx", later: (on) => `<Show when={${on}[0]()}><Dynamic component={${k}} /></Show>` },
});

export function flipReadContext(context, stmts, kit) {
  if (context === "mount-named") {
    const h = kit.name("f");
    return { hoisted: [decl(h, fn([...stmts, ret(domExpr("<i />"))])), expr(domExpr(`render(${h}, document.createElement("div"));`, ["render"]))], body: [], view: [] };
  }
  if (context === "arm-helper" || context === "arm-helper-chain") {
    const [h, g] = [kit.name("f"), kit.name("g")];
    const held = context === "arm-helper" ? [decl(h, fn(stmts))] : [decl(g, fn(stmts)), decl(h, fn([expr(call(ref(g)))]))];
    return { body: [...held, expr(domExpr(`if (isServer) ${h}();`, ["isServer"]))], view: [] };
  }
  if (context.startsWith("root-named-") || context === "root-two-handlers") {
    const h = kit.name("f");
    const held = decl(h, fn([expr(prim("root", fn(stmts)))]));
    switch (context) {
      case "root-named-timer": return { body: [held, expr(domExpr(`setTimeout(${h}, 0)`))], view: [] };
      case "root-named-then": return { body: [held, expr(domExpr(`Promise.resolve().then(${h})`))], view: [] };
      case "root-named-twice": return { body: [held, expr(call(ref(h))), expr(call(ref(h)))], view: [] };
      case "root-named-foreach": return { body: [held, expr(domExpr(`[1].forEach(${h})`))], view: [] };
      default: return { body: [held], view: [button(kit.name("b"), [expr(call(ref(h)))]), button(kit.name("b"), [expr(call(ref(h)))])] };
    }
  }
  if (CASTS[context]) {
    const { held, callee } = CASTS[context](kit.name("t"), kit.name("q"));
    const h = kit.name("f");
    return { body: [...held, decl(h, fn(stmts)), expr(domExpr(`${callee}(${h})`))], view: [] };
  }
  if (PATCHES[context]) {
    const [patch, callee] = PATCHES[context];
    const h = kit.name("f");
    return { hoisted: [expr(domExpr(patch))], body: [decl(h, fn(stmts)), expr(domExpr(`${callee}(${h})`))], view: [] };
  }
  if (context === "cleanup-mount" || context === "cleanup-mount-mixed" || context === "handler-mount") {
    const [k, d, b] = [kit.name("K"), kit.name("f"), kit.name("b")];
    const holder = decl(k, fn([...stmts, ret(domExpr("<i />"))]));
    const mount = `createRoot(() => <${k} />)`;
    if (context === "handler-mount") return { body: [holder], view: [button(b, [expr(domExpr(mount, undefined, ["createRoot"]))])] };
    const root = decl(d, domExpr(`createRoot((r) => { onCleanup(() => { ${mount}; }); return r; })`, undefined, ["createRoot", "onCleanup"]));
    return { body: [holder, root, ...(context === "cleanup-mount-mixed" ? [expr(domExpr(mount))] : [])], view: [button(b, [expr(domExpr(`${d}()`))])] };
  }
  if (context === "cast-late-foreach") {
    const t = kit.name("t");
    const h = kit.name("f");
    return { body: [decl(t, domExpr("{ forEach(f: () => void) { queueMicrotask(f); } }")), decl(h, fn(stmts)), expr(domExpr(`(${t} as unknown as number[]).forEach(${h})`))], view: [] };
  }
  // ADR 0681: `createEffect`'s apply has no owner, so this cleanup never registers and `K` never mounts.
  if (context === "apply-cleanup-mount") {
    const k = kit.name("K");
    const hold = `onCleanup(() => { render(() => <${k} />, document.createElement("div")); })`;
    return { body: [decl(k, fn([...stmts, ret(domExpr("<i />"))])), expr(domExpr(`createEffect(() => 1, () => { ${hold}; })`, ["render"], ["createEffect", "onCleanup"]))], view: [] };
  }
  // ADR 0683: the override runs the callback on a microtask, so the read has no window. The type says `number[]`.
  if (context === "subclass-late-foreach") {
    const [late, h, held] = [kit.name("L"), kit.name("f"), kit.name("a")];
    const fresh = `class ${late} extends Array<number> { override forEach(cb: (v: number, i: number, a: number[]) => void) { queueMicrotask(() => { super.forEach(cb); }); } }`;
    return { hoisted: [expr(domExpr(fresh))], body: [decl(h, fn(stmts)), decl(held, domExpr(`new ${late}(1) as number[]`)), expr(domExpr(`${held}.forEach(${h})`))], view: [] };
  }
  if (context === "arm-tag") {
    const k = kit.name("K");
    return { body: [decl(k, fn([...stmts, ret(domExpr("<i />"))]))], view: [mounted(kit.name("v"), domExpr(`{isServer ? <${k} /> : null}`, ["isServer"]))] };
  }
  if (context === "then-named") {
    const [f, m] = [kit.name("f"), kit.name("m")];
    const flight = domExpr(`new Promise<number>((done) => setTimeout(() => done(1), 1)).then(${f})`);
    return { body: [decl(f, fn([...stmts, ret(num(1))])), decl(m, prim("memo", fn([ret(flight)]))), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))], view: [] };
  }
  const held = holderForms("K")[context];
  if (held) {
    const k = kit.name("K");
    const form = holderForms(k)[context];
    const on = kit.name("on");
    const value = fn([...stmts, ret(num(1))]);
    const holder = decl(k, form.read === "body" ? value : fn([ret(jsxExpr(child(kit.name("r"), iife(value))))]));
    const web = ["Dynamic", "Portal", "dynamic"].filter((name) => (form.mount ?? form.later(on)).includes(name) || form.prelude?.includes(name));
    const solid = ["Show", "Loading"].filter((name) => (form.mount ?? form.later(on)).includes(`<${name}`));
    return {
      body: [
        holder,
        ...(form.later ? [decl(on, domExpr("createSignal(false)", undefined, ["createSignal"]))] : []),
      ],
      view: [
        ...(form.later ? [button(kit.name("b"), [expr(domExpr(`${on}[1](true)`))])] : []),
        mounted(kit.name("v"), domExpr(form.prelude ? `{(() => { ${form.prelude} return ${form.mount}; })()}` : form.mount ?? form.later(on), web, solid)),
      ],
    };
  }
  if (context.startsWith("on-")) {
    const t = kit.name("t");
    const form = loadingOnForms(t)[context];
    if (!form) return undefined;
    const first = form.first ? [decl(kit.name("w"), domExpr(`(() => { ${form.first} return 1; })()`))] : [];
    return {
      body: [decl(t, domExpr("createSignal(0)[0]", undefined, ["createSignal"])), ...form.body, ...first, decl(kit.name("l"), domExpr(form.tag, undefined, ["Loading"]))],
      view: [child(kit.name("v"), iife(fn([...stmts, ret(num(1))])))],
    };
  }
  return undefined;
}
