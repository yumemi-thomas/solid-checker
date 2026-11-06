// The flip's read track (scripts/flip/PLAN.md): recall contexts per runtime code, and the program of each. Empty at the base: a rule's
// commit adds its shapes here, so neither track edits misuse.mjs or the other's file.
//   FLIP_READ_CONTEXTS  { [code]: [context, ...] }, appended to that code's recallContexts
//   flipReadContext(context, stmts, kit)  the program, as a misuse.mjs context returns one, or undefined when the context is not this file's
//   FLIP_READ_LEGAL     { [code]: [context, ...] }, appended to that code's recallLegal
//   FLIP_READ_VARIANTS  { [code]: { [variant]: { kind, contexts } } }, a source of that kind, in those contexts, added to the code's recall variants
import { apiSignal, button, call, child, ctl, decl, domExpr, expr, fn, iife, jsxExpr, mounted, num, prim, ref, ret } from "./model.mjs";

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
  // ADR 0820: an iterator stepped from the compute, the site past the generator's first suspension.
  UNTRACKED_READ_AFTER_AWAIT: ["then-named", "gen-await", "gen-await-effect", "gen-yield", "gen-iife", "gen-wrap", "gen-delegate", "gen-next", "gen-sync-loop"],
  // ADR 0679: a named function holding a window-inheriting call, run directly from the body.
  STRICT_READ_UNTRACKED: ["root-named-twice", "root-named-foreach", ...LYING],
  // ADR 0782: a mount outside the cleanup, or in a handler, keeps the guard's throw.
  REACTIVE_WRITE_IN_OWNED_SCOPE: [...LYING, "cleanup-mount-mixed", "handler-mount"],
};

// ADR 0661, ADR 0668: quiet in Chrome. A mount after `render()` returned has no boundary check, and the source settled; a `<Dynamic>` body has
// no window, so a pending read retries. Under `<Loading>`: no placeless boundary warning leaves the program unjudged.
export const FLIP_READ_LEGAL = {
  // ADR 0820: the generator's first segment on the compute's stack, a tracked read; a loop that drains a sync generator before any suspension.
  UNTRACKED_READ_AFTER_AWAIT: ["gen-prefix", "gen-drain"],
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
// ADR 0790: a store write through a root no row of SC2003's flow described, on `createStore({ v, a: { v }, items })`. Each warns
// STORE_WRITE_OUTSIDE_SETTER in Chrome (rc.21); the build before certified each. In a handler or a timer: no strict read beside it.
const SUB_WRITE = { kind: "store-nested", contexts: ["handler", "timer"] };
const SUB_IDS = "const swId = <T,>(x: T): T => x; const swOuter = <T,>(y: T): T => swId(y);";
export const SUB_VALUE_WRITES = {
  "sw-loop-pattern": { ...SUB_WRITE, text: (o) => `for (const { a } of [${o}]) a.v = 7;` },
  "sw-decl-pattern": { ...SUB_WRITE, text: (o) => `const { a } = ${o}; a.v = 7;` },
  "sw-nested-identity": { ...SUB_WRITE, text: (o) => `${SUB_IDS} swOuter(${o}).v = 7;` },
  "sw-called-literal": { ...SUB_WRITE, text: (o) => `((x: typeof ${o}) => x)(${o}).v = 7;` },
  "sw-entries": { ...SUB_WRITE, text: (o) => `for (const [, x] of Object.entries({ k: ${o}.a })) x.v = 7;` },
  "sw-from-entries": { ...SUB_WRITE, text: (o) => `Object.fromEntries(Object.entries({ k: ${o}.a })).k.v = 7;` },
  "sw-conditional": { ...SUB_WRITE, text: (o) => `(Math.random() >= 0 ? ${o} : { v: 0, a: { v: 0 }, items: [] }).v = 7;` },
  "sw-array-root": { ...SUB_WRITE, text: (o) => `[${o}.a][0].v = 7;` },
  "sw-let": { ...SUB_WRITE, text: (o) => `let r = { v: 0 }; r = ${o}.a; r.v = 7;` },
  "sw-param-pattern": { ...SUB_WRITE, text: (o) => `const w = ({ a }: { a: { v: number } }) => { a.v = 7; }; w(${o});` },
  "sw-rest": { ...SUB_WRITE, text: (o) => `const w = (...xs: { v: number }[]) => { xs[0].v = 7; }; w(${o}.a);` },
  "sw-delete-call": { ...SUB_WRITE, text: (o) => `${SUB_IDS} delete (swId(${o}.a) as { v?: number }).v;` },
  "sw-await": { ...SUB_WRITE, text: (o) => `void (async () => { (await Promise.resolve(${o}.a)).v = 7; })();` },
  "sw-then": { ...SUB_WRITE, text: (o) => `void Promise.resolve(${o}.a).then((x) => { x.v = 7; });` },
  "sw-map-get": { ...SUB_WRITE, text: (o) => `new Map([[1, ${o}.a]]).get(1)!.v = 7;` },
  // patterns api/v09-store-write-nullish and values/d6-comma: a `??` and a comma whose value is the store (root writes).
  "sw-coalesce": { ...SUB_WRITE, text: (o) => `(((((): any => null)()) as typeof ${o} | null) ?? ${o}).v = 7;` },
  "sw-comma": { ...SUB_WRITE, text: (o) => `(Math.random(), ${o}).v = 7;` },
  // ADR 0791: a delete behind an optional chain, tsc-clean, warns.
  "sw-delete-optional-chain": { ...SUB_WRITE, kind: "store-opt", text: (o) => `delete ${o}?.v;` },
  // ADR 0829: a computed key, an array index, an open key: tsc-clean, warn; the build before certified each (P8).
  "sw-delete-computed": { ...SUB_WRITE, kind: "store-opt", text: (o) => `delete ${o}["v"];` },
  "sw-delete-index": { ...SUB_WRITE, text: (o) => `delete ${o}.items[0];` },
  "sw-delete-open-key": { ...SUB_WRITE, text: (o) => `const k: number = 0; delete ${o}.items[k];` },
  // ADR 0830: a delete tsc rejects (TS2703 under an assertion, TS2790 on a required member) under a directive: tsc reports
  // nothing, the proxy warns. The build before certified the first two (and, before ADR 0829, the third).
  "sw-delete-nonnull-ignored": { ...SUB_WRITE, kind: "store-opt", text: (o) => `// @ts-ignore\n${AUX}delete ${o}.v!;` },
  "sw-delete-as-expect-error": { ...SUB_WRITE, kind: "store-opt", text: (o) => `// @ts-expect-error\n${AUX}delete (${o}.v as number | undefined);` },
  "sw-delete-required-ignored": { ...SUB_WRITE, text: (o) => `// @ts-ignore\n${AUX}delete ${o}.a.v;` },
  // ADR 0790, research-sc9012.md section 2's t5, t13, t22, t33, dl8: roots no row described, each U now.
  "sw-object-root": { ...SUB_WRITE, text: (o) => `({ s: ${o}.a }).s.v = 7;` },
  "sw-generator": { ...SUB_WRITE, text: (o) => `function* swGen() { yield ${o}.a; } for (const x of swGen()) x.v = 7;` },
  "sw-assign-copy": { ...SUB_WRITE, text: (o) => `Object.assign({}, ${o}).a.v = 7;` },
  "sw-param-array-pattern": { ...SUB_WRITE, text: (o) => `const w = ([e]: { v: number }[]) => { e.v = 7; }; w([${o}.a]);` },
  "sw-foreach-pattern": { ...SUB_WRITE, text: (o) => `[${o}].forEach(({ a }) => { a.v = 7; });` },
  // Rediscovery 214-219, ADRs 0716, 0717, 0719, 0720: a loop head, an aliased standard write, a plain parameter, and a call's result or
  // a loop's element. The build before ADR 0716 (1232c2f2c^) certified each.
  "sw-loop-head": { ...SUB_WRITE, text: (o) => `for (${o}.v of [2]) { }` },
  "sw-assign-alias": { ...SUB_WRITE, text: (o) => `const as = Object.assign; as(${o}, { v: 2 });` },
  "sw-param-plain": { ...SUB_WRITE, text: (o) => `const w = (x: { v: number }) => { x.v = 7; }; w(${o}.a);` },
  "sw-identity": { ...SUB_WRITE, text: (o) => `const swId1 = <T,>(x: T): T => x; swId1(${o}).v = 7;` },
  "sw-find": { ...SUB_WRITE, text: (o) => `[${o}.a].find(() => true)!.v = 7;` },
  "sw-for-of": { ...SUB_WRITE, text: (o) => `for (const e of [${o}.a]) e.v = 7;` },
  "sw-filter": { ...SUB_WRITE, text: (o) => `[${o}.a].filter(() => true)[0].v = 7;` },
  "sw-values": { ...SUB_WRITE, text: (o) => `Object.values({ k: ${o}.a })[0].v = 7;` },
  // Quiet in Chrome: a fresh literal written at its own property, and the frame's plain argument. The build before reported the first
  // a violation: the conditional's entity named its first token, the store in its test.
  "sw-fresh-test": { ...SUB_WRITE, contexts: [], legal: ["handler", "timer"], text: (o) => `(${o}.v > 0 ? { v: 1 } : { v: 2 }).v = 7;` },
  "sw-fresh-spread": { ...SUB_WRITE, contexts: [], legal: ["handler", "timer"], text: (o) => `(Math.random() >= 0 ? { ...${o}.a } : { v: 0 }).v = 7;` },
  "sw-fresh-frame": { ...SUB_WRITE, contexts: [], legal: ["handler", "timer"], text: (o) => `${SUB_IDS} swOuter({ v: 1 }).v = 7; void swOuter(${o});` },
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
  // W's census review H3: a tracked position excused a pending accessor reached through a pattern over a holder.
  "net-pattern-pending": { ...PEEK, text: (o) => `const v = { c: ${o} }; const { c: h } = v; void h();` },
};
// ADR 0802: no fact calls these sources async, and each is pending by what its compute reads. Chrome warns on each; each certified before.
const OPEN_MEMO = "const useM = (g: () => number) => createMemo(() => g());";
export const OPEN_COMPUTES = {
  "open-hook-memo": { ...PEEK, imports: ["createMemo"], text: (o) => `${OPEN_MEMO} const m = useM(${o}); void m();` },
  "open-hook-object": { ...PEEK, imports: ["createMemo"], text: (o) => `const useM = (g: () => number) => { const m = createMemo(() => g()); return { m }; }; const { m } = useM(${o}); void m();` },
  "open-hook-tuple": { ...PEEK, imports: ["createMemo"], text: (o) => `const useM = (g: () => number) => { const m = createMemo(() => g()); return [m] as const; }; const [m] = useM(${o}); void m();` },
  "open-hook-let": { ...PEEK, imports: ["createMemo"], text: (o) => `${OPEN_MEMO} let m = useM(${o}); m = useM(${o}); void m();` },
  "open-hook-box": { ...PEEK, imports: ["createMemo"], text: (o) => `${OPEN_MEMO} const box = { m: useM(${o}) }; void box.m();` },
  "open-memo-chain": { ...PEEK, imports: ["createMemo"], text: (o) => `${OPEN_MEMO} const m = useM(${o}); const d = createMemo(() => m() + 1); void d();` },
  "open-signal-fn": { ...PEEK, imports: ["createSignal"], text: (o) => `const [s] = createSignal(() => ${o}()); void s();` },
  "open-store-fn": { ...PEEK, imports: ["createStore"], text: (o) => `const [s] = createStore(() => ({ v: ${o}() }), { v: 0 }); void s.v;` },
};
// ADR 0801: a hook's tuple slot is the accessor it names. Quiet, and each answered SC9012 before; the proof must not lose them.
const nowHook = (imports, setup) => ({ importLines: ['import { useNow } from "./now-hook";'], files: { "now-hook.ts": `import { ${imports} } from "solid-js";\nexport function useNow(interval = 50) {\n  const [now, setNow] = createSignal(new Date());\n  ${setup}\n  return [now] as const;\n}\n` } });
const SETTLED_SLOT = { kind: "plain", contexts: [], legal: ["jsx", "show"], text: () => "const [now] = useNow(); void now();" };
export const SETTLED_SLOTS = {
  "settled-slot": { ...SETTLED_SLOT, ...nowHook("createSignal", "") },
  "settled-slot-writer": { ...SETTLED_SLOT, ...nowHook("createSignal, onSettled", "onSettled(() => { const t = setInterval(() => setNow(new Date()), interval); return () => clearInterval(t); });") },
};
// ADR 0801: the same slot over a pending source is a proved read of it.
export const PENDING_SLOTS = {
  "pending-slot": { ...PEEK, text: (o) => `const useM = () => [${o}] as const; const [m] = useM(); void m();` },
};
// ADR 0800: a value's walk has no length limit. Each catches the pending read through helpers; the build before stopped at four hops.
const chain = (hops) => (o) => `const f1 = () => ${o}(); ${Array.from({ length: hops - 1 }, (_, i) => `const f${i + 2} = () => f${i + 1}();`).join(" ")} try { f${hops}(); } catch { }`;
const CAUGHT = { kind: "async", contexts: ["memo", "effect-compute"] };
export const CAUGHT_CHAINS = {
  "caught-chain-5": { ...CAUGHT, text: chain(5) },
  "caught-chain-9": { ...CAUGHT, text: chain(9) },
};
const STORE_CALL = { kind: "store-array", contexts: ["body", "handler"] };
export const NET_STORE_CALLS = {
  "net-push-bind": { ...STORE_CALL, text: (o) => `Array.prototype.push.bind(${o})(1);` },
  "net-push-held": { ...STORE_CALL, text: (o) => `const f = Array.prototype.push.bind(${o}); f(1);` },
  "net-push-reflect": { ...STORE_CALL, text: (o) => `Reflect.apply(Array.prototype.push, ${o}, [1]);` },
  "net-push-apply-held": { ...STORE_CALL, text: (o) => `const more = [1]; Array.prototype.push.apply(${o}, more);` },
  // Rediscovery 217, ADRs 0688 and 0773: `sort` on a store array through `Function.call`, `bind` or `Reflect.apply`. A descending
  // comparator on `[1, 2]`, so the sort writes.
  "net-sort-call": { ...STORE_CALL, kind: "store-list", text: (o) => `Array.prototype.sort.call(${o}.items, (x: number, y: number) => y - x);` },
  "net-sort-bind": { ...STORE_CALL, kind: "store-list", text: (o) => `Array.prototype.sort.bind(${o}.items)((x: number, y: number) => y - x);` },
  "net-sort-reflect": { ...STORE_CALL, kind: "store-list", text: (o) => `Reflect.apply(Array.prototype.sort, ${o}.items, [(x: number, y: number) => y - x]);` },
};

// Rediscovery 202, ADR 0676: a read in a store setter's callback in an apply, the setter a module store's.
const SETTER_STORE = { importLines: ['import { setSw2 } from "./sw2";'], files: { "sw2.ts": 'import { createStore } from "solid-js";\nexport const [sw2, setSw2] = createStore({ n: 0 });\n' } };
export const SETTER_READS = {
  "setter-fn-apply": { ...SETTER_STORE, kind: "plain", contexts: ["effect-apply"], text: (o) => `setSw2((d) => { d.n = ${o}(); });` },
};

// ADR 0692: the census. Each hands the accessor to a position no row followed, then reads it. The generated program's component is an
// `export function`: at module scope a test would prove nothing about the export arm, whose span is the whole statement.
const USE_READ = { kind: "plain", contexts: ["body", "effect-apply", "root"] };
export const VALUE_USES = {
  "use-let": { ...USE_READ, text: (o) => `let h = ${o}; void h();` },
  "use-member": { ...USE_READ, text: (o) => `const b: { h?: () => number } = {}; b.h = ${o}; void b.h();` },
  "use-logical": { ...USE_READ, text: (o) => `const h = ${o} || ${o}; void h();` },
  "use-comma": { ...USE_READ, text: (o) => `const h = (void 0, ${o}); void h();` },
  "use-class": { ...USE_READ, text: (o) => `class H { t = ${o}; } void new H().t();` },
  "use-new": { ...USE_READ, text: (o) => `class H { t: () => number; constructor(t: () => number) { this.t = t; } } void new H(${o}).t();` },
  "use-default": { ...USE_READ, text: (o) => `const f = (h = ${o}) => h(); void f();` },
  "use-map": { ...USE_READ, text: (o) => `const m = new Map([["k", ${o}]]); void m.get("k")!();` },
  "use-array-at": { ...USE_READ, text: (o) => `const a = [${o}]; void a.at(0)!();` },
  "use-bound": { ...USE_READ, text: (o) => `const b = ${o}.bind(null); void b();` },
  "use-tagged": { ...USE_READ, text: (o) => `const t = (_: TemplateStringsArray, h: () => number) => h(); void t\`\${${o}}\`;` },
  "use-spread": { ...USE_READ, text: (o) => `((h: () => number) => void h())(...([${o}] as [() => number]));` },
  "use-iife": { ...USE_READ, text: (o) => `const h = (() => ${o})(); void h();` },
  "use-nested": { ...USE_READ, text: (o) => `const w = { v: { v: ${o} } }; void w.v.v();` },
  "use-forward": { ...USE_READ, text: (o) => `const f0 = (g: () => number) => g(); const f1 = (g: () => number) => f0(g); void f1(${o});` },
  "use-root": { ...USE_READ, imports: ["createRoot"], text: (o) => `const h = createRoot(() => ${o}); void h();` },
  "use-untrack": { ...USE_READ, imports: ["untrack"], text: (o) => `const h = untrack(() => ${o}); void h();` },
  "use-yield": { ...USE_READ, text: (o) => `function* g() { yield ${o}; } void g().next().value!();` },
  "use-yield-delegate": { ...USE_READ, text: (o) => `function* g() { yield ${o}; } function* g2() { yield* g(); } void g2().next().value!();` },
  "use-chosen": { ...USE_READ, text: (o) => `const useA = () => ({ read: ${o} }); const hook = Math.random() >= 0 ? useA : useA; void hook().read();` },
  // The default, reached by no table: 1c's forms.
  "use-throw": { ...USE_READ, text: (o) => `try { throw ${o}; } catch (e) { void (e as () => number)(); }` },
  "use-throw-operand": { ...USE_READ, text: (o) => `try { void (1 + (() => { throw ${o}; })()); } catch (e) { void (e as () => number)(); }` },
  "use-throw-hook": { ...USE_READ, text: (o) => `const u = () => ${o}; try { throw u(); } catch (e) { void (e as () => number)(); }` },
  "use-throw-created": { ...USE_READ, imports: ["createMemo"], text: (o) => `void ${o}; try { throw createMemo(() => 1); } catch (e) { void (e as () => number)(); }` },
  // A `Followed` that was no proof: no binding or row names the call or the tag.
  "use-class-return": { ...USE_READ, text: (o) => `class K { f() { return ${o}; } } void new K().f()();` },
  "use-pattern-hop": { ...USE_READ, text: (o) => `const v = { c: ${o} }; const { c: h } = v; void h();` },
  "use-slot-return": { ...USE_READ, text: (o) => `const v = { f: () => ${o} }; void v.f()();` },
  "use-fn-alias": { ...USE_READ, text: (o) => `const f = () => ${o}; const g = f; void g()();` },
  "use-tag": { ...USE_READ, text: (o) => `const H = ${o} as unknown as () => null; void (<H />);` },
  // P13: no binding, so no reference starts the walk.
  "use-created-values": { ...USE_READ, imports: ["createMemo"], text: (o) => `void ${o}; void (Object.values({ a: createMemo(() => 1) })[0] as () => number)();` },
  "use-created-json": { ...USE_READ, imports: ["createStore"], text: (o) => `void ${o}; void JSON.stringify(createStore({ a: 1 })[0]);` },
  // W's census review: H1, `export default` of a literal or a creating call, whose importers nothing walked. The export is App's own and
  // a helper module imports it, so the obligation lands in App.tsx, the only file the grid judge reads.
  "use-export-default-array": { ...USE_READ, importLines: ['const [dxc] = createSignal(1);', 'export default [dxc] as const;', 'import { readDxa } from "./dxa";'], files: { "dxa.ts": 'import d from "./App";\nexport const readDxa = () => { try { throw d[0]; } catch (e) { void (e as () => number)(); } };\n' }, text: (o) => `void ${o}; readDxa();` },
  "use-export-default-object": { ...USE_READ, importLines: ['const [dxc] = createSignal(1);', 'export default { c: dxc };', 'import { readDxo } from "./dxo";'], files: { "dxo.ts": 'import d from "./App";\nexport const readDxo = () => { try { throw d.c; } catch (e) { void (e as () => number)(); } };\n' }, text: (o) => `void ${o}; readDxo();` },
  "use-export-default-created": { ...USE_READ, imports: ["createMemo"], importLines: ['export default createMemo(() => 1);', 'import { readDxm } from "./dxm";'], files: { "dxm.ts": 'import d from "./App";\nexport const readDxm = () => { try { throw d; } catch (e) { void (e as () => number)(); } };\n' }, text: (o) => `void ${o}; readDxm();` },
  // H2: inside a parameter hop, a pattern over a holder needed no row.
  "use-param-pattern": { ...USE_READ, text: (o) => `const run = (f: () => number) => { const v = { f }; const { f: g } = v; return g(); }; void run(${o});` },
  // H4: a store root's member reached through a holder slot needed no row.
  "use-store-holder-member": { ...USE_READ, imports: ["createStore"], text: (o) => `void ${o}; const [st] = createStore({ a: 1 }); const v = { st }; void v.st.a;` },
  // H5: a bare child is what its parent does with it; `For` calls it as the mapper.
  "use-child-for": { ...USE_READ, imports: ["For"], text: (o) => `const el = <For each={[1]}>{${o}}</For>; void (el as unknown as () => unknown)();` },
  // P12 (R2b's held patterns): a store a hook returns, read through the hook's call; and a `let` export of an accessor.
  "use-hook-store-member": { ...USE_READ, imports: ["createStore"], importLines: ['const [hsm] = createStore({ v: 1 });', 'const useHsm = () => hsm;'], text: (o) => `void ${o}; void useHsm().v;` },
  // The hook is App's own export and another module reads through it, so the obligation lands in App.tsx, the file the grid judge reads.
  "use-hook-store-module": { ...USE_READ, imports: ["createStore"], importLines: ['const [hse] = createStore({ v: 1 });', 'export function useHse() { return hse; }', 'import { readHse } from "./hse";'], files: { "hse.ts": 'import { useHse } from "./App";\nexport const readHse = () => useHse().v;\n' }, text: (o) => `void ${o}; void readHse();` },
  "use-export-let": { ...USE_READ, importLines: ['const [elc] = createSignal(1);', 'export let elRead = elc;', 'import { readEl } from "./el";'], files: { "el.ts": 'import { elRead } from "./App";\nexport const readEl = () => elRead();\n' }, text: (o) => `void ${o}; void readEl();` },
};
const USE_WRITE = { ...HOST, contexts: ["body", "memo"] };
export const VALUE_WRITES = {
  "use-write-throw": { ...USE_WRITE, text: () => `${SET} try { throw w; } catch (e) { (e as (v: number) => void)(1); }` },
  "use-write-let": { ...USE_WRITE, text: () => `${SET} let g = w; g(1);` },
  "use-write-map": { ...USE_WRITE, text: () => `${SET} new Map([["k", w]]).get("k")!(1);` },
};
// ADR 0821: a `const` copy of a source another module exports. Every reference to it is outside the file that holds the source.
const EXPORT_COPY = {
  kind: "plain",
  importLines: ['import { copied, copiedStore } from "./export-copy";'],
  files: { "export-copy.ts": 'import { createSignal, createStore } from "solid-js";\nconst [c] = createSignal(1);\nconst [st] = createStore({ v: 1 });\nexport const copied = c;\nexport const copiedStore = st;\n' },
};
export const EXPORT_COPY_READS = {
  "export-copy-signal": { ...EXPORT_COPY, contexts: ["body", "root"], legal: ["memo", "effect-compute"], text: () => "void copied();" },
  "export-copy-store": { ...EXPORT_COPY, contexts: ["body", "root"], legal: ["memo", "effect-compute"], text: () => "void copiedStore.v;" },
};
export const EXPORT_COPY_WRITES = {
  "export-copy-write": { ...EXPORT_COPY, contexts: ["body", "handler"], text: () => "copiedStore.v = 2;" },
};

// misuse.mjs `AUX`: in a site text over lines, it marks the line the runtime frame sits in (print.mjs `auxLine`).
const AUX = "@@AUX@@";
// ADR 0830: the one-argument `createEffect` is TS2554, which a directive silences; Chrome then throws MISSING_EFFECT_FN.
const SUPPRESSED_EFFECT = { kind: "plain", contexts: ["body", "root", "handler"], imports: ["createEffect"] };
export const SUPPRESSED_EFFECTS = {
  "one-arg-ts-ignore": { ...SUPPRESSED_EFFECT, text: () => `// @ts-ignore\n${AUX}createEffect(() => 1);` },
  "one-arg-expect-error": { ...SUPPRESSED_EFFECT, text: () => `// @ts-expect-error\n${AUX}createEffect(() => 1);` },
};

export const FLIP_READ_VARIANTS = {
  MISSING_EFFECT_FN: SUPPRESSED_EFFECTS,
  PENDING_ASYNC_UNTRACKED_READ: { "open-any": { kind: "open-any", contexts: ["body", "root"] }, "open-notready": { kind: "open-notready", contexts: ["body", "root"] } },
  STRICT_READ_UNTRACKED: { ...HANDED_READS, ...CREATED_READS, ...ABSENT_GUARD, ...NET_READS, ...STAR_AS_READS, ...SETTER_READS, ...VALUE_USES, ...EXPORT_COPY_READS },
  REACTIVE_WRITE_IN_OWNED_SCOPE: { ...NET_WRITES, ...VALUE_WRITES },
  ACTION_CALLED_IN_OWNED_SCOPE: NET_ACTIONS,
  ASYNC_OUTSIDE_LOADING_BOUNDARY: { ...NET_PENDING, ...OPEN_COMPUTES, ...SETTLED_SLOTS, ...PENDING_SLOTS },
  CAUGHT_REACTIVE_READ: CAUGHT_CHAINS,
  STORE_WRITE_OUTSIDE_SETTER: { ...NET_STORE_CALLS, ...STAR_AS_WRITES, ...SUB_VALUE_WRITES, ...EXPORT_COPY_WRITES },
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
  if (context.startsWith("gen-")) {
    const effect = context === "gen-await-effect";
    const body = [ctl(effect ? "gen-await" : context, stmts), ret(num(1))];
    if (effect) return { body: [expr(prim("effect", fn(body, { async: true }), { apply: fn([]) }))], view: [] };
    const m = kit.name("m");
    return { body: [decl(m, prim("memo", fn(body, { async: true }))), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))], view: [] };
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
