// The flip's read track (scripts/flip/PLAN.md): recall contexts per runtime code, and the program of each. Empty at the base: a rule's
// commit adds its shapes here, so neither track edits misuse.mjs or the other's file.
//   FLIP_READ_CONTEXTS  { [code]: [context, ...] }, appended to that code's recallContexts
//   flipReadContext(context, stmts, kit)  the program, as a misuse.mjs context returns one, or undefined when the context is not this file's
//   FLIP_READ_LEGAL     { [code]: [context, ...] }, appended to that code's recallLegal
//   FLIP_READ_VARIANTS  { [code]: { [variant]: { kind, contexts } } }, a source of that kind, in those contexts, added to the code's recall variants
import { apiSignal, awaitStmt, button, call, child, ctl, decl, domExpr, expr, flush, fn, iife, jsx, jsxExpr, member, mounted, num, prim, ref, ret } from "./model.mjs";

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
  // ADR 0865: an async generator's iterator is an AsyncIterable: the name reaches the checker as a declaration, not an async literal.
  "cmp-async-gen": 'async function* () { yield 1; }, { sync: true }',
  "cmp-async-gen-named": "cmpGen, { sync: true }",
  "cmp-async-gen-member": "() => cmpHolder.g(), { sync: true }",
  "cmp-async-iter-class": "() => new CmpIter(), { sync: true }",
};

export const SYNC_COMPUTE_LINES = {
  "cmp-async-gen-named": "async function* cmpGen() { yield 1; }",
  "cmp-async-gen-member": "const cmpHolder = { async *g() { yield 1; } };",
  "cmp-async-iter-class": "class CmpIter { async *[Symbol.asyncIterator]() { yield 1; } }",
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
const SYNC_PROTO = "function (this: any, f: any) { f(); return this; }";
const PATCH_KEYS = 'const pk = ["Pro", "mise"].join(""), pm = ["th", "en"].join("");';
const PATCHES = {
  "patch-global": ["(globalThis as any).setTimeout = (f: () => void) => { f(); return 0; };", "setTimeout"],
  "patch-window": ["window.setTimeout = ((f: () => void) => { f(); return 0; }) as any;", "setTimeout"],
  "patch-define": ['Object.defineProperty(globalThis, "setTimeout", { value: (f: () => void) => { f(); return 0; } });', "setTimeout"],
  "patch-proto": ["Promise.prototype.then = function (this: any, f: any) { f(); return this; } as any;", "Promise.resolve().then"],
  "patch-proto-alias": ["const proto = Promise.prototype; proto.then = function (this: any, f: any) { f(); return this; } as any;", "Promise.resolve().then"],
  // ADR 0841: the prototype or the global reached by a key no fact names, or by `Object.getPrototypeOf`.
  "patch-key-proto": [`${PATCH_KEYS} (globalThis as any)[pk].prototype[pm] = ${SYNC_PROTO};`, "Promise.resolve().then"],
  "patch-getproto": [`const gp = Object.getPrototypeOf(Promise.resolve()); gp.then = ${SYNC_PROTO};`, "Promise.resolve().then"],
  "patch-assign-key-proto": [`${PATCH_KEYS} Object.assign((globalThis as any)[pk].prototype, { [pm]: ${SYNC_PROTO} });`, "Promise.resolve().then"],
  "patch-key-alias": [`${PATCH_KEYS} const gC = (globalThis as any)[pk]; gC.prototype.then = ${SYNC_PROTO};`, "Promise.resolve().then"],
  "patch-key-ctor": [`${PATCH_KEYS} let gA = false; const gR = (globalThis as any)[pk]; (globalThis as any)[pk] = class extends gR { static resolve(v?: unknown) { return gA ? ((gA = false), { then(f: () => void) { f(); } }) : gR.resolve(v); } };`, "(gA = true, Promise.resolve()).then"],
};
// ADR 0840: the impostor away from the receiver (a parameter, a field, a return, a generic, a member), and with no assertion at all
// (`any`, an array's covariance, a type guard, an own `then` put on a real promise). Each `then` runs the callback on the caller's stack.
const RUN_THEN = "(p: Promise<void>, f: () => void) => { p.then(f); }";
const CASTS = {
  "cast-then": (t) => ({ held: [decl(t, domExpr(SYNC_THEN))], callee: `(${t} as unknown as Promise<void>).then` }),
  "cast-any-then": (t) => ({ held: [decl(t, domExpr(`${SYNC_THEN} as any`))], callee: `(${t} as Promise<void>).then` }),
  "cast-held-then": (t, q) => ({ held: [decl(t, domExpr(SYNC_THEN)), decl(q, domExpr(`${t} as unknown as Promise<void>`))], callee: `${q}.then` }),
  "cast-param": (t, q) => ({ held: [decl(t, domExpr(SYNC_THEN)), decl(q, domExpr(RUN_THEN))], call: (h) => `${q}(${t} as any, ${h})` }),
  "cast-field": (t, q) => ({ hoisted: [expr(domExpr(`class ${q} { p = ${SYNC_THEN} as unknown as Promise<void>; }`))], held: [], callee: `new ${q}().p.then` }),
  "cast-return": (t) => ({ held: [decl(t, domExpr(`(): Promise<void> => (${SYNC_THEN}) as unknown as Promise<void>`))], callee: `${t}().then` }),
  "cast-generic": (t, q) => ({ hoisted: [expr(domExpr(`function ${q}<T>(x: unknown): T { return x as T; }`))], held: [decl(t, domExpr(`${q}<Promise<void>>(${SYNC_THEN})`))], callee: `${t}.then` }),
  "cast-nested": (t) => ({ held: [decl(t, domExpr(`{ p: ${SYNC_THEN} } as unknown as { p: Promise<void> }`))], callee: `${t}.p.then` }),
  "cast-any-param": (t, q) => ({ held: [expr(domExpr(`const ${t}: any = ${SYNC_THEN}`)), decl(q, domExpr(RUN_THEN))], call: (h) => `${q}(${t}, ${h})` }),
  "cast-covariance": (t, q) => ({ hoisted: [expr(domExpr(`const ${t}: Promise<void>[] = []; const ${q}: (Promise<void> | { then(f: () => void): void })[] = ${t}; ${q}.push(${SYNC_THEN})`))], held: [], callee: `${t}[0].then` }),
  "cast-guard": (t, q) => ({ hoisted: [expr(domExpr(`function ${q}(x: unknown): x is Promise<void> { return typeof x === "object" && x !== null && "then" in x; }`))], held: [expr(domExpr(`const ${t}: unknown = ${SYNC_THEN}`))], call: (h) => `if (${q}(${t})) ${t}.then(${h})` }),
  "cast-instance": (t) => ({ held: [decl(t, domExpr("Promise.resolve()")), expr(domExpr(`Object.assign(${t}, { then: (f: () => void) => { f(); } })`))], callee: `${t}.then` }),
};
const LYING = [...Object.keys(CASTS), ...Object.keys(PATCHES)];
// ADR 0874: a receiver the census took for the standard library's, with the program's own member on it. Each runs the callback at once
// on the caller's stack. The event type names the program's own: the runtime's listeners keep the built-in.
const ADD_SYNC = 'function (this: any, t: string, cb: any, ...r: any[]) { if (t === "x-sync") cb(); else (hO as any).call(this, t, cb, ...r); }';
const HOST_CALL = (target) => (h) => `${target}.addEventListener("x-sync", ${h})`;
const HOST_HOLD = "const hO = EventTarget.prototype.addEventListener;";
const IN_RUN = '/\\bC\\d+\\b/.test(new Error().stack ?? "")';
const RECEIVERS = {
  // R10c: a host global's own member, written under its lowercase spelling, through an alias, or handed to a helper.
  "recv-doc-patch": () => ({ hoisted: [expr(domExpr(`${HOST_HOLD} (document as any).addEventListener = ${ADD_SYNC};`))], held: [], call: HOST_CALL("document") }),
  "recv-win-patch": () => ({ hoisted: [expr(domExpr(`${HOST_HOLD} (window as any).addEventListener = ${ADD_SYNC};`))], held: [], call: HOST_CALL("window") }),
  "recv-body-patch": () => ({ hoisted: [expr(domExpr(`${HOST_HOLD} (document.body as any).addEventListener = ${ADD_SYNC};`))], held: [], call: HOST_CALL("document.body") }),
  "recv-doc-alias": (t) => ({ hoisted: [expr(domExpr(`${HOST_HOLD} const ${t}: any = document; ${t}.addEventListener = ${ADD_SYNC};`))], held: [], call: HOST_CALL("document") }),
  "recv-doc-handed": (t) => ({ hoisted: [expr(domExpr(`${HOST_HOLD} const ${t} = (o: any) => { o.addEventListener = ${ADD_SYNC}; }; ${t}(document);`))], held: [], call: HOST_CALL("document") }),
  "recv-target-proto": () => ({ hoisted: [expr(domExpr(`${HOST_HOLD} EventTarget.prototype.addEventListener = ${ADD_SYNC} as any;`))], held: [], call: HOST_CALL("document") }),
  // Review D7: the object replaced under its members: a new prototype, by call or by a `__proto__` write, or `self` itself.
  "recv-doc-setproto": () => ({ hoisted: [expr(domExpr(`${HOST_HOLD} Object.setPrototypeOf(document, Object.create(Object.getPrototypeOf(document), { addEventListener: { value: ${ADD_SYNC} } }));`))], held: [], call: HOST_CALL("document") }),
  "recv-doc-dunder": () => ({ hoisted: [expr(domExpr(`${HOST_HOLD} (document as any).__proto__ = Object.create(Object.getPrototypeOf(document), { addEventListener: { value: ${ADD_SYNC} } });`))], held: [], call: HOST_CALL("document") }),
  "recv-self-fake": () => ({ hoisted: [expr(domExpr(`${HOST_HOLD} (globalThis as any).self = { addEventListener: ${ADD_SYNC} };`))], held: [], call: HOST_CALL("self") }),
  // Review D10: the withheld call's result is the function: `find` hands back the handler, which the program calls.
  "recv-result": (t, q) => ({ hoisted: [expr(domExpr(`const ${t}: unknown[] = []; ${t}[Number("0")] = () => 0;`))], held: [decl(q, domExpr("(x: (() => void)[]) => x"))], call: (h) => `${q}([${h}]).find(() => true)?.()` }),
  // ADR 0875: the program's only computed key is a same-file `const` bound to `Symbol(...)`, which names no member: the receiver's `forEach`
  // is the standard one, run in place (Chrome warns). The build before took the key for `*` and withheld every census member.
  "recv-symbol-const": (t, q) => ({ hoisted: [expr(domExpr(`const ${t}k = Symbol("k"); const ${t}o: any = {}; ${t}o[${t}k] = () => 0;`))], held: [decl(q, domExpr("(x: number[]) => x"))], callee: `${q}([1]).forEach` }),
  "recv-symbol-for-const": (t, q) => ({ hoisted: [expr(domExpr(`const ${t}k = Symbol.for("k"); const ${t}o: any = {}; ${t}o[${t}k] = () => 0;`))], held: [decl(q, domExpr("(x: number[]) => x"))], callee: `${q}([1]).forEach` }),
  // R10b: a promise's species, the prototype's constructor replaced by a class the standard's own then builds from.
  "recv-species-promise": (t) => ({ hoisted: [expr(domExpr(`class ${t} { static get [Symbol.species](): unknown { return ${IN_RUN} ? ${t} : Promise; } constructor(run: (a: unknown, b: unknown) => void) { run(() => {}, () => {}); } then(f: () => void) { f(); return this; } } (Promise.prototype as any).constructor = ${t};`))], held: [], callee: "Promise.resolve().then(() => 0).then" }),
};
// The same, quiet in Chrome: the checker took the program's own late method for the standard's in-place one. Each hands the callback to a microtask.
const LATE_EACH = "(cb: () => void) => { queueMicrotask(cb); }";
const LATE_RECEIVERS = {
  // R10a: an array method hands the receiver to its callback, and a method the program put on the prototype gets it as this.
  "recv-handed-late": (t) => ({ hoisted: [expr(domExpr(`const ${t} = [1]; ${t}.map((_v, _i, a) => { (a as any).forEach = ${LATE_EACH}; return 0; });`))], held: [], callee: `${t}.forEach` }),
  "recv-reduce-late": (t) => ({ hoisted: [expr(domExpr(`const ${t} = [1, 2]; ${t}.reduce((c, _v, _i, a) => { (a as any).forEach = ${LATE_EACH}; return c; }, 0);`))], held: [], callee: `${t}.forEach` }),
  "recv-this-late": (t) => ({ hoisted: [expr(domExpr(`(Array.prototype as any).hand = function (this: any) { this.forEach = ${LATE_EACH}; }; const ${t} = [1]; (${t} as any).hand();`))], held: [], callee: `${t}.forEach` }),
  // Review D7: a listed method the program replaced runs with the array as this.
  "recv-slice-late": (t) => ({ hoisted: [expr(domExpr(`const ${t}o = Array.prototype.slice; (Array.prototype as any).slice = function (this: any, ...a: any[]) { if (this[0] === 7) this.forEach = ${LATE_EACH}; return ${t}o.apply(this, a); }; const ${t} = [7]; ${t}.slice();`))], held: [], callee: `${t}.forEach` }),
  // Review R33: a member that answers with its receiver hands it on.
  "recv-answer-late": (t) => ({ hoisted: [expr(domExpr(`const ${t} = [1, 2]; const ${t}y: any = ${t}.reverse(); ${t}y.forEach = ${LATE_EACH};`))], held: [], call: (h) => `${t}.forEach(() => { ${h}(); })` }),
  // R10b: an array's species, as a promise's above.
  "recv-species-array": (t) => ({ hoisted: [expr(domExpr(`class ${t} { static get [Symbol.species](): unknown { return ${IN_RUN} ? ${t} : Array; } forEach(f: () => void) { queueMicrotask(f); } } (Array.prototype as any).constructor = ${t};`))], held: [], callee: "[1].map((v) => v).forEach" }),
  // ADR 0875: a key that only looks like a symbol puts the program's late `forEach` on the array: a string from a symbol
  // (`.description`, `Symbol.keyFor`), a shadowed `Symbol`, a `let` rebound to a string. The build before read the first two as symbols.
  "recv-symbol-desc-late": (t) => ({ hoisted: [expr(domExpr(`const ${t}: any = [1]; ${t}[Symbol("forEach").description!] = ${LATE_EACH};`))], held: [], callee: `(${t} as number[]).forEach` }),
  "recv-symbol-keyfor-late": (t) => ({ hoisted: [expr(domExpr(`const ${t}: any = [1]; ${t}[Symbol.keyFor(Symbol.for("forEach"))!] = ${LATE_EACH};`))], held: [], callee: `(${t} as number[]).forEach` }),
  "recv-symbol-desc-const-late": (t) => ({ hoisted: [expr(domExpr(`const ${t}k = Symbol("forEach").description!; const ${t}: any = [1]; ${t}[${t}k] = ${LATE_EACH};`))], held: [], callee: `(${t} as number[]).forEach` }),
  "recv-symbol-let-late": (t) => ({ hoisted: [expr(domExpr(`let ${t}k: any = Symbol("k"); ${t}k = "forEach"; const ${t}: any = [1]; ${t}[${t}k] = ${LATE_EACH};`))], held: [], callee: `(${t} as number[]).forEach` }),
  // Review D15: `Symbol` replaced in a second module the program imports for its effect; `Symbol.species` gives the array a class
  // whose `forEach` is late (`Symbol.species` names a member's builder, never a symbol that replaces no member).
  "recv-symbol-patched-module-late": (t) => ({ importLines: ['import "./symbol-patch";'], files: { "symbol-patch.ts": '(globalThis as any).Symbol = (_: string) => "forEach";\nexport {};\n' }, hoisted: [expr(domExpr(`const ${t}k = Symbol("k"); const ${t}: any = [1]; ${t}[${t}k] = ${LATE_EACH};`))], held: [], callee: `(${t} as number[]).forEach` }),
  "recv-symbol-for-patched-module-late": (t) => ({ importLines: ['import "./symbol-for-patch";'], files: { "symbol-for-patch.ts": '(Symbol as any).for = (_: string) => "forEach";\nexport {};\n' }, hoisted: [expr(domExpr(`const ${t}k = Symbol.for("k"); const ${t}: any = [1]; ${t}[${t}k] = ${LATE_EACH};`))], held: [], callee: `(${t} as number[]).forEach` }),
  "recv-symbol-species-late": (t) => ({ hoisted: [expr(domExpr(`const ${t}k = Symbol.species; class ${t}A extends Array { forEach(f: (...a: any[]) => void): void { queueMicrotask(() => f()); } } Object.defineProperty(Array, ${t}k, { configurable: true, get: () => ${t}A });`))], held: [], callee: "[1].map((v) => v).forEach" }),
  "recv-symbol-shadow-late": (t) => ({ hoisted: [expr(domExpr(`const ${t}: any = [1]; { const Symbol = (_: string) => "forEach"; const ${t}k = Symbol("k"); ${t}[${t}k] = ${LATE_EACH}; }`))], held: [], callee: `(${t} as number[]).forEach` }),
  // R10d: replace dispatches through the first argument's Symbol.replace.
  "recv-replace-late": (t) => ({ hoisted: [expr(domExpr(`const ${t}: any = { [Symbol.replace]: (s: string, cb: () => void) => { queueMicrotask(cb); return s; } };`))], held: [], call: (h) => `"x".replace(${t}, ${h} as any)` }),
};
// ADR 0803: the iterator written back unchanged. The tripwires fire and the write still runs (Chrome warns): never a silent clean.
const ITER_SAME = { "iter-same": "Array.prototype[Symbol.iterator] = Array.prototype[Symbol.iterator];" };

// ADR 0803: each spelling patches array iteration at module scope, before the component, so `const [v, set] = createX()` binds a no-op as
// `set` and the write in the body never runs (Chrome is quiet). The primitive's own slots are then uncertifiable, never a violation.
// Only while a generated component runs (`C<n>` on the stack): the host page's own code keeps the built-in behaviour.
const ITER_IN = '/\\bC\\d+\\b/.test(new Error().stack ?? "")';
const ITER_PAIR = `function (this: unknown[]) { return itO.call(this.length === 2 && typeof this[1] === "function" && (this[1] as Function).name !== "noop" && ${ITER_IN} ? [this[0], function noop() {}] : this); }`;
const ITER_NEXT = `function (this: object) { const r = itN.call(this); const k = itSeen.get(this) ?? 0; itSeen.set(this, k + 1); if (!r.done && k === 1 && typeof r.value === "function" && r.value.name !== "noop" && ${ITER_IN}) return { value: function noop() {}, done: false }; return r; }`;
// The same patch as code from a string (plain JavaScript, no annotations).
const ITER_CODE = `const itO = Array.prototype[Symbol.iterator]; Array.prototype[Symbol.iterator] = function () { return itO.call(this.length === 2 && typeof this[1] === "function" && this[1].name !== "noop" && /\\bC\\d+\\b/.test(new Error().stack || "") ? [this[0], function noop() {}] : this); };`;
const ITER_IP = "const itIP = Object.getPrototypeOf([][Symbol.iterator]()); const itN = itIP.next; const itSeen = new WeakMap<object, number>();";
const ITER_PATCHES = {
  "iter-key": `const itKs: PropertyKey[] = [Symbol.iterator]; const itO = Array.prototype[Symbol.iterator]; (Array.prototype as any)[itKs[0]] = ${ITER_PAIR};`,
  "iter-assign-src": `const itO = Array.prototype[Symbol.iterator]; const itSrc: object = { [Symbol.iterator]: ${ITER_PAIR} }; Object.assign(Array.prototype, itSrc);`,
  "iter-define-key": `const itKs: PropertyKey[] = [Symbol.iterator]; const itO = Array.prototype[Symbol.iterator]; Object.defineProperty(Array.prototype, itKs[0], { value: ${ITER_PAIR}, configurable: true, writable: true });`,
  "iter-reflect-set": `const itO = Array.prototype[Symbol.iterator]; Reflect.set(Array.prototype, Symbol.iterator, ${ITER_PAIR});`,
  "iter-alias": `const itAP = Array.prototype; const itO = itAP[Symbol.iterator]; itAP[Symbol.iterator] = ${ITER_PAIR};`,
  "iter-alias-reflect": `const itAP = Array.prototype; const itO = itAP[Symbol.iterator]; Reflect.set(itAP, Symbol.iterator, ${ITER_PAIR});`,
  "iter-next": `${ITER_IP} itIP.next = ${ITER_NEXT};`,
  "iter-next-assign": `${ITER_IP} Object.assign(itIP, { next: ${ITER_NEXT} });`,
  "iter-next-define": `${ITER_IP} Object.defineProperty(itIP, "next", { value: ${ITER_NEXT}, configurable: true, writable: true });`,
  "iter-next-key": `${ITER_IP} const itKs: PropertyKey[] = ["ne" + "xt"]; itIP[itKs[0]] = ${ITER_NEXT};`,
  "iter-next-src": `${ITER_IP} const itSrc: object = { next: ${ITER_NEXT} }; Object.assign(itIP, itSrc);`,
  "iter-alias-key": `const itAP: any = Array.prototype; const itKs: PropertyKey[] = [Symbol.iterator]; const itO = itAP[Symbol.iterator]; itAP[itKs[0]] = ${ITER_PAIR};`,
  // ADR 0803, item 3: the hidden side (ADR 0801's census). Code from a string, run by a `Function` held in a parameter (pattern s03) or
  // called on the spot; `Array.prototype` handed to a helper that writes it under keys it is given.
  "iter-fn-escaped": `const itF: unknown = globalThis.Function; const itRun = (make: (code: string) => () => void) => { make(${JSON.stringify(ITER_CODE)})(); }; itRun(itF as (code: string) => () => void);`,
  "iter-fn-new": `new Function(${JSON.stringify(ITER_CODE)})();`,
  "iter-handed-proto": `const itSet = (p: any, k: PropertyKey, f: unknown) => { p[k] = f; }; const itO = Array.prototype[Symbol.iterator]; itSet(Array.prototype, Symbol.iterator, ${ITER_PAIR});`,
  // Review a9fc #1, #2: Array.prototype spelled through an array literal, a global alias, or a destructured getter, then written under a
  // key no literal names.
  "iter-gpo-key": `const itAP: any = Object.getPrototypeOf([]); const itKs: PropertyKey[] = [Symbol.iterator]; const itO = itAP[itKs[0]]; itAP[itKs[0]] = ${ITER_PAIR};`,
  "iter-proto-key": `const itAP: any = ([] as any).__proto__; const itKs: PropertyKey[] = [Symbol.iterator]; const itO = itAP[itKs[0]]; itAP[itKs[0]] = ${ITER_PAIR};`,
  "iter-ctor-key": `const itAP: any = [].constructor.prototype; const itKs: PropertyKey[] = [Symbol.iterator]; const itO = itAP[itKs[0]]; itAP[itKs[0]] = ${ITER_PAIR};`,
  "iter-global-alias": `const itG: any = globalThis; const itAP = itG.Array.prototype; const itKs: PropertyKey[] = [itG.Symbol.iterator]; const itO = itAP[itKs[0]]; itAP[itKs[0]] = ${ITER_PAIR};`,
  "iter-gpo-destructure": `const { getPrototypeOf: itGp } = Object; const itAP: any = itGp([]); const itKs: PropertyKey[] = [Symbol.iterator]; const itO = itAP[itKs[0]]; itAP[itKs[0]] = ${ITER_PAIR};`,
};

// ADR 0857: the census's `handed-flush` (misuse.mjs) through other entries. A function a tracked JSX expression runs (by a call, or by a
// standard-library call that runs it inline) runs under the render effect's observer, and `flush(fn)` runs `fn` there. Then the legal
// twins: a handler runs the same entry with no observer, an empty receiver runs nothing, and `<K />` renders the function as a component.
const HANDED_FLUSH = ["handed-flush-each", "handed-flush-call", "named-flush-call", "flush-iife"];
const HANDED_FLUSH_LEGAL = ["handed-flush-handler", "handed-flush-empty", "handed-flush-tag"];
// ADR 0857's entry walk ends at its bound (review R13): the site is N named functions from tracked JSX. Twice-called helpers end the in-place
// climb at once; the sole-call chain outruns the climb's own 16 layers. Past either bound the row is open, never the site's lexical role.
const ENTRY_CHAINS = { "entry-chain-5": [5, 2], "entry-chain-13": [13, 2], "sole-chain-17": [17, 1] };
// ADR 0859: `flush(fn)` keeps the observer, for a read in a layer `fn` runs too. In a named helper, the helper's caller decides: tracked JSX is
// silent, the component body warns. An IIFE on the way is no reason to take the read for the body's.
const FLUSH_READ_BODY = ["flush-read-body", "flush-read-body-iife"];
const FLUSH_READ_LEGAL = ["flush-read-each", "flush-read-each-iife", "flush-read-call", "flush-read-call-iife"];
// Review R15: a helper that runs the literal in place and keeps it: the in-place run is tracked, the kept one runs in the body.
const FLUSH_READ_STORED = "flush-read-stored";
// Review R16: an `async` function's prefix runs on its caller's stack; past its await it runs on no observer.
const ASYNC_ENTRY = "async-entry";

// ADRs 0870-0873: where the after-await walk had no row. The branch runs (the condition holds at run time; no fact says so), so the read follows
// the await on the path that runs.
const P_AWAIT = "await Promise.resolve()";
const ON = "Math.random() >= 0";
const OFF = "Math.random() < 0";
const COND_AWAITS = {
  "aw-if": `if (${ON}) ${P_AWAIT};`,
  "aw-and": `${ON} && (${P_AWAIT});`,
  "aw-or": `${OFF} || (${P_AWAIT});`,
  "aw-nullish": `let awN: number | undefined; awN ??= (${P_AWAIT}, 1); void awN;`,
  "aw-ternary": `${ON} ? ${P_AWAIT} : 0;`,
  "aw-switch": `switch (${ON} ? 1 : 2) { case 1: ${P_AWAIT}; }`,
  "aw-dstr": `const { awA = (${P_AWAIT}, 1) } = {} as { awA?: number }; void awA;`,
  "aw-label": `awL: { if (${OFF}) break awL; ${P_AWAIT}; }`,
  "aw-optional": `const awO = ((_: unknown) => 1) as ((v: unknown) => number) | undefined; awO?.(${P_AWAIT});`,
};
// Review D12: a wrapper that runs its parameter in place but hands its result on (`(f) => f()`, `async (f) => await f()`), or runs it past an
// await of its own: the compute's flight carries the read. `aw-inplace-stmt` discards the result in a synchronous memo: the read is on the
// function's own flight.
const IN_PLACE_USED = ["aw-inplace-return", "aw-inplace-await", "aw-inplace-late"];
// B6-B8 (ADRs 0879-0882): a promise handler on the compute's flight the walk did not read: a member (`then(api.fmt)`), an `async` function, a
// receiver cast away from or to `any`. `aw-catch-member` is a rejection handler on a fulfilled promise: it never runs.
const THEN_HANDLERS = {
  "aw-then-member": (h, o) => `Promise.resolve(1).then(${o}.fmt)`,
  "aw-then-async": (h) => `Promise.resolve(1).then(${h})`,
  "aw-then-cast": (h) => `(Promise.resolve(1) as unknown as Promise<number>).then(${h})`,
  "aw-then-any": (h) => `(Promise.resolve(1) as any).then(${h})`,
  // Legal (review, D-R3d-2): a sync handler that calls an async function and drops nothing it awaits; Chrome (rc.21) is silent at the
  // read past that function's own await, on 16 of 16 ids.
  "aw-then-calls-async": (h) => `Promise.resolve(1).then(() => ${h}())`,
};
// The await's arm cannot precede the site (the other arm of an `if`), or leaves before it runs: the site runs with no await before it.
const COND_AWAITS_LEGAL = ["aw-else", "aw-else-flip", "aw-early"];
const HELPER_AWAITS = ["aw-shared-after", "aw-shared-before", "aw-helper-late", "aw-wrapper", "aw-wrapper-2", ...IN_PLACE_USED];


// ADR 0893: a plain throw in a computation that stores its error (a memo and the other `Creates` computes) is seen only when a reader takes it
// where it is reported. `STORED_READS` holds the site in a memo and varies the reader; `STORED_SITES` moves the site. All measured on rc.21 Chrome.
const solid = (text, ...names) => domExpr(text, undefined, names);
const effectReading = (text, arms = "() => {}") => expr(solid(`createEffect(() => ${text}, ${arms});`, "createEffect"));
const HANDLED = "fallback={(e) => <i>{String(e())}</i>}";
const LOGGED = "fallback={() => <i>x</i>}";
const under = (fallback, inner) => solid(`<Errored ${fallback}>${inner}</Errored>`, "Errored");
const STORED_READS = {
  "st-unread": () => ({}),
  "st-chain-unread": (m, kit) => ({ body: [decl(kit.name("m"), prim("memo", fn([ret(call(ref(m)))])))] }),
  "st-chain-read": (m, kit) => { const n = kit.name("m"); return { body: [decl(n, prim("memo", fn([ret(call(ref(m)))]))), effectReading(`${n}()`)] }; },
  "st-read-effect": (m) => ({ body: [effectReading(`${m}()`)] }),
  "st-read-try": (m) => ({ body: [expr(solid(`createEffect(() => { try { return ${m}(); } catch { return 0; } }, () => {});`, "createEffect"))] }),
  "st-read-caller-try": (m, kit) => { const r = kit.name("r"); return { body: [decl(r, domExpr(`() => ${m}()`)), expr(solid(`createEffect(() => { try { return ${r}(); } catch { return 0; } }, () => {});`, "createEffect"))] }; },
  "st-read-helper": (m, kit) => { const r = kit.name("r"); return { body: [decl(r, domExpr(`() => ${m}()`)), effectReading(`${r}()`)] }; },
  "st-read-ispending": (m) => ({ body: [expr(solid(`createEffect(() => isPending(() => ${m}()), () => {});`, "createEffect", "isPending"))] }),
  "st-read-latest": (m) => ({ body: [expr(solid(`createEffect(() => latest(() => ${m}()), () => {});`, "createEffect", "latest"))] }),
  "st-read-untrack": (m) => ({ body: [expr(solid(`createEffect(() => untrack(() => ${m}()), () => {});`, "createEffect", "untrack"))] }),
  "st-read-err-arm": (m) => ({ body: [effectReading(`${m}()`, "{ effect: () => {}, error: () => {} }")] }),
  "st-read-await": (m) => ({ body: [expr(domExpr(`void (async () => { await 0; ${m}(); })();`))] }),
  "st-read-then": (m) => ({ body: [expr(domExpr(`void Promise.resolve(1).then(() => ${m}());`))] }),
  "st-read-then-catch": (m) => ({ body: [expr(domExpr(`void Promise.resolve(1).then(() => ${m}()).catch(() => {});`))] }),
  "st-read-handler": (m, kit) => ({ view: [button(kit.name("b"), [expr(call(ref(m)))])] }),
  "st-read-jsx": (m, kit) => ({ view: [mounted(kit.name("v"), domExpr(`{${m}()}`))] }),
  "st-read-attr": (m, kit) => ({ view: [mounted(kit.name("v"), domExpr(`<i title={String(${m}())}></i>`))] }),
  "st-read-errored-handled": (m, kit) => ({ view: [mounted(kit.name("v"), under(HANDLED, `<i>{${m}()}</i>`))] }),
  "st-read-errored-logged": (m, kit) => ({ view: [mounted(kit.name("v"), under(LOGGED, `<i>{${m}()}</i>`))] }),
  "st-read-errored-element": (m, kit) => ({ view: [mounted(kit.name("v"), under("fallback={<i>x</i>}", `<i>{${m}()}</i>`))] }),
  "st-read-getter-ignored": (m, kit) => { const k = kit.name("K"); return { body: [decl(k, domExpr("(props: { v: number }) => <i></i>"))], view: [mounted(kit.name("v"), domExpr(`<${k} v={${m}()} />`))] }; },
  "st-read-getter": (m, kit) => { const k = kit.name("K"); return { body: [decl(k, domExpr("(props: { v: number }) => <i>{props.v}</i>"))], view: [mounted(kit.name("v"), domExpr(`<${k} v={${m}()} />`))] }; },
  "st-read-show-false": (m, kit) => ({ view: [mounted(kit.name("v"), solid(`<Show when={false}><i>{${m}()}</i></Show>`, "Show"))] }),
  "st-read-show-true": (m, kit) => ({ view: [mounted(kit.name("v"), solid(`<Show when={true}><i>{${m}()}</i></Show>`, "Show"))] }),
};
const storedForm = (name, tail, value) => (stmts) => prim("memo", fn([...stmts, ret(value ?? num(1))]), { creator: { name, tail } });
const OBJECT_VALUE = domExpr("{ a: 1 }");
const STORED_FORMS = {
  "st-c-deferred": storedForm("createDeferred", ""),
  "st-c-signal": storedForm("createSignal", ""),
  "st-c-optimistic": storedForm("createOptimistic", ""),
  "st-c-store": storedForm("createStore", ", { a: 0 }", OBJECT_VALUE),
  "st-c-optimistic-store": storedForm("createOptimisticStore", ", { a: 0 }", OBJECT_VALUE),
  "st-c-projection": storedForm("createProjection", ", { a: 0 }", OBJECT_VALUE),
  "st-c-children": storedForm("children", ""),
};
const helperOf = (stmts, kit) => { const h = kit.name("f"); return [h, decl(h, fn([...stmts, ret(num(1))]))]; };
const STORED_SITES = {
  "st-helper-effect": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); return { body: [held, decl(kit.name("m"), domExpr(`createMemo(() => ${h}())`, undefined, ["createMemo"])), effectReading(`${h}()`)], view: [] }; },
  "st-helper-body": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); return { body: [held, decl(kit.name("m"), domExpr(`createMemo(() => ${h}())`, undefined, ["createMemo"])), expr(domExpr(`${h}();`))], view: [] }; },
  // R36: a reference is no run, and what runs a handed function may take its throw.
  "st-ref-only": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); return { body: [held, decl(kit.name("c"), domExpr(h))], view: [] }; },
  "st-handed-catch": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); const t = kit.name("t"); return { body: [held, decl(t, domExpr("(g: () => unknown) => { try { g(); } catch { } }")), effectReading(`(${t}(${h}), 1)`)], view: [] }; },
  "st-prop-catch": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); const k = kit.name("K"); return { body: [held, decl(k, domExpr("(p: { run: () => unknown }) => { try { p.run(); } catch { } return <i></i>; }"))], view: [mounted(kit.name("v"), domExpr(`<${k} run={() => ${h}()} />`))] }; },
  // R37: a promise handler's throw rejects a promise something may hold.
  "st-then-catch": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); return { body: [held, expr(domExpr(`void Promise.resolve(1).then(${h}).catch(() => {});`))], view: [] }; },
  "st-then-await-try": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); return { body: [held, expr(domExpr(`void (async () => { try { await Promise.resolve(1).then(${h}); } catch { } })();`))], view: [] }; },
  // R34: a render or tracked effect's throw escalates to the boundary above; a user effect's is logged past it.
  "st-render-handled": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); const k = kit.name("K"); return { body: [held, decl(k, solid(`() => { createRenderEffect(() => ${h}(), () => {}); return <i></i>; }`, "createRenderEffect"))], view: [mounted(kit.name("v"), under(HANDLED, `<${k} />`))] }; },
  "st-tracked-handled": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); const k = kit.name("K"); return { body: [held, decl(k, solid(`() => { createTrackedEffect(() => { ${h}(); }); return <i></i>; }`, "createTrackedEffect"))], view: [mounted(kit.name("v"), under(HANDLED, `<${k} />`))] }; },
  "st-user-handled": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); const k = kit.name("K"); return { body: [held, decl(k, solid(`() => { createEffect(() => ${h}(), () => {}); return <i></i>; }`, "createEffect"))], view: [mounted(kit.name("v"), under(HANDLED, `<${k} />`))] }; },
  "st-render-plain": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); return { body: [held, expr(solid(`createRenderEffect(() => ${h}(), () => {});`, "createRenderEffect"))], view: [] }; },
  "st-alias-unread": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); const c = kit.name("c"); return { body: [held, decl(c, domExpr(h)), decl(kit.name("m"), domExpr(`createMemo(${c})`, undefined, ["createMemo"]))], view: [] }; },
  // ADR 0893: a throw the program catches fires no diagnostic, around the site or a caller.
  "st-try-site": (stmts) => ({ body: [expr(prim("effect", fn([ctl("try-catch", stmts), ret(num(1))]), { apply: fn([]) }))], view: [] }),
  "st-try-caller": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); return { body: [held, expr(solid(`createEffect(() => { try { return ${h}(); } catch { return 0; } }, () => {});`, "createEffect"))], view: [] }; },
  "st-effect-err-arm": (stmts) => ({ body: [expr(prim("effect", fn([...stmts, ret(num(1))]), { apply: fn([]), errorBundle: true }))], view: [] }),
  "st-helper-err-arm": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); return { body: [held, effectReading(`${h}()`, "{ effect: () => {}, error: () => {} }")], view: [] }; },
  "st-jsx-handled": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); return { body: [held], view: [mounted(kit.name("v"), under(HANDLED, `<i>{${h}()}</i>`))] }; },
  "st-jsx-logged": (stmts, kit) => { const [h, held] = helperOf(stmts, kit); return { body: [held], view: [mounted(kit.name("v"), under(LOGGED, `<i>{${h}()}</i>`))] }; },
  "st-body-handled": (stmts, kit) => { const k = kit.name("K"); return { body: [decl(k, fn([...stmts, ret(domExpr("<i></i>"))]))], view: [mounted(kit.name("v"), under(HANDLED, `<${k} />`))] }; },
  "st-child-memo-handled": (stmts, kit) => { const k = kit.name("K"); const m = kit.name("m"); return { body: [decl(k, fn([decl(m, prim("memo", fn([...stmts, ret(num(1))]))), ret(domExpr(`<i>{${m}()}</i>`))]))], view: [mounted(kit.name("v"), under(HANDLED, `<${k} />`))] }; },
};
const STORED_ASYNC = {
  "st-async-unread": () => ({}),
  "st-async-read": (m) => ({ body: [effectReading(`${m}()`)] }),
};
/** ADR 0893: every shape above, by the name its row carries. */
export const STORED_CONTEXTS = [...Object.keys(STORED_READS), ...Object.keys(STORED_FORMS).flatMap((form) => [`${form}-unread`, ...(["st-c-deferred", "st-c-children"].includes(form) ? [`${form}-read`] : [])]), ...Object.keys(STORED_SITES)];
export const STORED_ASYNC_CONTEXTS = Object.keys(STORED_ASYNC);
const STORED_THROWS = ["RESOLVE_IN_REACTIVE_SCOPE", "UNTIL_IN_REACTIVE_SCOPE", "DYNAMIC_STATIC_PROMISE", "CONTEXT_NOT_FOUND"];

export const FLIP_READ_CONTEXTS = {
  LOADING_ON_ACCESSOR: ["on-any", "on-unknown", "on-spread", "on-json", "on-prop", "on-call"],
  // ADR 0662, ADR 0668: a `<Dynamic>` body has no strict window and still warns with no boundary; a `<Portal>` is a plain synchronous mount.
  ASYNC_OUTSIDE_LOADING_BOUNDARY: ["dyn-jsx-body", "portal-jsx", "portal-dyn", ...HANDED_FLUSH],
  RESOLVE_IN_REACTIVE_SCOPE: [...HANDED_FLUSH, ...Object.keys(ENTRY_CHAINS), ASYNC_ENTRY],
  UNTIL_IN_REACTIVE_SCOPE: [...HANDED_FLUSH, ...Object.keys(ENTRY_CHAINS), ASYNC_ENTRY],
  // ADR 0838 (R2h): "jsx", a caught read a carrier runs in tracked JSX (CAUGHT_HANDED_READS only).
  CAUGHT_REACTIVE_READ: [...HANDED_FLUSH, "jsx"],
  PENDING_ASYNC_UNTRACKED_READ: ["portal-body", "dyn-jsx-mixed", "apply-placed-twin"],
  // ADR 0835 review R1/R6: a helper's read placed at one call (tracked, under `<Loading>`) and cut on another path (a chain past the walk's
  // bound in `onSettled`): the cut path's claim needs a row of its own.
  PENDING_ASYNC_FORBIDDEN_SCOPE: ["settled-chain-placed", "export-placed"],
  // ADR 0669: a memo's promise chain with a handler handed by name.
  // ADR 0820: an iterator stepped from the compute, the site past the generator's first suspension.
  UNTRACKED_READ_AFTER_AWAIT: ["then-named", "then-withheld", "then-withheld-named", "gen-await", "gen-await-effect", "gen-yield", "gen-iife", "gen-wrap", "gen-delegate", "gen-next", "gen-sync-loop", ...Object.keys(COND_AWAITS), ...HELPER_AWAITS, ...Object.keys(THEN_HANDLERS).filter((c) => c !== "aw-then-calls-async")],
  // ADR 0679: a named function holding a window-inheriting call, run directly from the body.
  STRICT_READ_UNTRACKED: ["root-named-twice", "root-named-foreach", FLUSH_READ_STORED, ...FLUSH_READ_BODY, ...LYING, ...Object.keys(RECEIVERS), ...Object.keys(ITER_SAME)],
  // ADR 0782: a mount outside the cleanup, or in a handler, keeps the guard's throw.
  REACTIVE_WRITE_IN_OWNED_SCOPE: [...LYING, ...Object.keys(RECEIVERS), "cleanup-mount-mixed", "handler-mount", ...Object.keys(ITER_SAME)],
};

// ADR 0893, measured on rc.21 Chrome over every code here: silent with the error stored unread, in a `try`, under a handling boundary, an effect's
// `error` arm or `isPending`. `st-helper-body` throws in the body for a code that throws on every run; `resolve` and `until` need an observer.
const STORED_SILENT = ["st-unread", "st-chain-unread", "st-read-try", "st-read-caller-try", "st-read-ispending", "st-read-err-arm", "st-read-errored-handled", "st-read-getter-ignored", "st-read-show-false",
  "st-read-then-catch", "st-prop-catch", "st-ref-only", "st-handed-catch", "st-then-catch", "st-then-await-try", "st-render-handled", "st-tracked-handled", "st-alias-unread", "st-try-site", "st-try-caller", "st-effect-err-arm", "st-helper-err-arm", "st-jsx-handled", "st-body-handled", "st-child-memo-handled", ...Object.keys(STORED_FORMS).map((form) => `${form}-unread`)];
const observerOnly = (code) => code === "RESOLVE_IN_REACTIVE_SCOPE" || code === "UNTIL_IN_REACTIVE_SCOPE";
const storedSilent = (code) => [...STORED_SILENT, ...(observerOnly(code) ? ["st-helper-body"] : [])];
for (const code of STORED_THROWS) FLIP_READ_CONTEXTS[code] = [...(FLIP_READ_CONTEXTS[code] ?? []), ...STORED_CONTEXTS.filter((context) => !storedSilent(code).includes(context))];
FLIP_READ_CONTEXTS.NO_OWNER_CONTEXT = [...(FLIP_READ_CONTEXTS.NO_OWNER_CONTEXT ?? []), "st-async-read"];

// ADR 0661, ADR 0668: quiet in Chrome. A mount after `render()` returned has no boundary check, and the source settled; a `<Dynamic>` body has
// no window, so a pending read retries. Under `<Loading>`: no placeless boundary warning leaves the program unjudged.
export const FLIP_READ_LEGAL = {
  // ADR 0820: the generator's first segment on the compute's stack, a tracked read; a loop that drains a sync generator before any suspension.
  UNTRACKED_READ_AFTER_AWAIT: ["gen-prefix", "gen-drain", "await-object-then", ...COND_AWAITS_LEGAL, "aw-wrapper-untracked", "aw-inplace-stmt", "aw-catch-member", "aw-then-calls-async"],
  ASYNC_OUTSIDE_LOADING_BOUNDARY: ["show-false-later", "ternary-later", "and-later", "dyn-jsx-later"],
  PENDING_ASYNC_UNTRACKED_READ: ["dyn-jsx-body-l", "dyn-fn-body-l"],
  // Review R6's twins with the other path in a handler: only the placed call reads during a pass.
  PENDING_ASYNC_FORBIDDEN_SCOPE: ["handler-chain-placed", "export-placed-l"],
  // ADR 0678: `render(Local, el)` calls `Local` under a root, with no strict window. ADR 0677: a client build never runs a helper only `isServer` reaches.
  // ADR 0781: `cast-late-foreach` hands the callback to a microtask, whatever the assertion says.
  // ADR 0874, review R30-R32: a withheld call's callback proves no run: the program's own `forEach` queues it.
  FLUSH_IN_ACTION: ["wh-action-late"],
  STRICT_READ_UNTRACKED: ["wh-helper-late", "mount-named", "arm-helper", "arm-helper-chain", "arm-tag", "root-named-timer", "root-named-then", "root-two-handlers", "apply-cleanup-mount", "subclass-late-foreach", "cast-late-foreach", ...FLUSH_READ_LEGAL, ...Object.keys(LATE_RECEIVERS)],
  // ADR 0782: `cleanup-mount`'s only mount is in a cleanup, where the guard stands down.
  // ADR 0349 with ADR 0282: the patched iteration hands back a no-op setter, so the write never runs.
  REACTIVE_WRITE_IN_OWNED_SCOPE: ["wh-root-cleanup", "arm-helper", "arm-helper-chain", "arm-tag", "cast-late-foreach", "cleanup-mount", ...Object.keys(LATE_RECEIVERS), ...Object.keys(ITER_PATCHES)],
  STORE_WRITE_OUTSIDE_SETTER: ["arm-helper"],
  RESOLVE_IN_REACTIVE_SCOPE: [...HANDED_FLUSH_LEGAL, "entry-chain-handler", "async-entry-late"],
  UNTIL_IN_REACTIVE_SCOPE: [...HANDED_FLUSH_LEGAL, "entry-chain-handler", "async-entry-late"],
};

for (const code of STORED_THROWS) FLIP_READ_LEGAL[code] = [...(FLIP_READ_LEGAL[code] ?? []), ...storedSilent(code)];
FLIP_READ_LEGAL.NO_OWNER_CONTEXT = [...(FLIP_READ_LEGAL.NO_OWNER_CONTEXT ?? []), "st-async-unread", "st-then-catch"];

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
  // ADR 0860: the store escapes at module scope, and the site writes through what holds it.
  "sw-map-apart": { ...SUB_WRITE, imports: ["createStore"], importLines: ["const [swm] = createStore({ v: 1 });", 'const swmm = new Map([["s", swm]]);'], text: (o) => `void ${o}; swmm.get("s")!.v = 7;` },
  "sw-nullish-apart": { ...SUB_WRITE, imports: ["createStore"], importLines: ["const [swn] = createStore({ v: 1 });", "const swn2 = ((): typeof swn | null => null)() ?? swn;"], text: (o) => `void ${o}; swn2.v = 7;` },
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
  // Review item 10: an element inserts and calls a bare accessor child, so a pending one is read there; only a cast spells it.
  "net-bare-child": { ...PEEK, text: (o) => `void (<b>{${o} as unknown as string}</b>);` },
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
// Review R18: a probe reads the source too; a first load throws from `latest`/`isPending` to the surrounding boundary (legal under `<Loading>`).
export const PROBE_READS = {
  "probe-latest": { ...PEEK, contexts: ["jsx"], imports: ["latest"], text: (o) => `void latest(() => ${o}());` },
  "probe-is-pending": { ...PEEK, contexts: ["jsx"], imports: ["isPending"], text: (o) => `void isPending(() => ${o}());` },
};
// ADR 0838 (R2h): a pending read in a `try` whose catch swallows the throw, in a function a carrier runs. The boundary never sees the throw
// (Chrome rc.21: CAUGHT only); the carrier's position was the only place the `try` was looked for.
const CAUGHT_HANDED = { kind: "async", only: true, contexts: ["jsx"] };
export const CAUGHT_HANDED_READS = {
  "caught-handed-foreach": { ...CAUGHT_HANDED, text: (o) => `const hf = () => { try { void ${o}(); } catch {} }; [1].forEach(hf);` },
  "caught-handed-map": { ...CAUGHT_HANDED, text: (o) => `const hf = () => { try { void ${o}(); } catch {} }; void [1].map(hf);` },
  "caught-handed-call": { ...CAUGHT_HANDED, text: (o) => `const hf = () => { try { void ${o}(); } catch {} }; hf.call(undefined);` },
};
// R2h (patterns/flip/sync-memo-jsx-read): a `sync: true` async memo read in tracked JSX. Chrome rc.21 reports SYNC_NODE_RECEIVED_ASYNC
// only: the node errors, it is never pending, so no boundary claim holds; the build before proved ASYNC_OUTSIDE_LOADING_BOUNDARY there.
export const SYNC_NODE_READS = {
  "sync-memo-read": { kind: "plain", contexts: ["jsx"], bindings: ["direct"], imports: ["createMemo"], text: () => "const sm = createMemo(async () => 1, { sync: true }); void sm();" },
};
// Review R17: an exported helper holds the read, one local call is placed (tracked, under `<Loading>`), and another module calls it: no walk here sees
// that caller. `other` runs it in `onSettled` (Chrome: PENDING_ASYNC_FORBIDDEN_SCOPE) or in a handler never clicked. The source is module-level, so only this variant takes these contexts.
const exportedHelperFile = (run) => ({ "eo.tsx": `import { onSettled } from "solid-js";\nimport { eRead } from "./App";\nexport function EO() {\n  ${run}\n}\n` });
export const EXPORT_READS = {
  "export-read": {
    kind: "async", only: true, bindings: ["direct", "alias"], contexts: ["export-placed"], imports: ["createMemo"], text: () => "eA()",
    importLines: ['const eA = createMemo(async () => { await new Promise((r) => setTimeout(r, 10)); return 1; });', 'import { EO } from "./eo";'],
    files: exportedHelperFile("onSettled(() => { eRead(); });\n  return null;"),
  },
  "export-read-handler": {
    kind: "async", only: true, bindings: ["direct", "alias"], contexts: [], legal: ["export-placed-l"], imports: ["createMemo"], text: () => "eA()",
    importLines: ['const eA = createMemo(async () => { await new Promise((r) => setTimeout(r, 10)); return 1; });', 'import { EO } from "./eo";'],
    files: exportedHelperFile('void onSettled;\n  return <button onClick={() => eRead()}>x</button>;'),
  },
};
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
  // Review item 5: a pattern over a store root reached through a holder slot needed no row.
  "use-store-holder-pattern": { ...USE_READ, imports: ["createStore"], text: (o) => `void ${o}; const [st] = createStore({ a: 1 }); const v = { st }; const { a } = v.st; void a;` },
  // Review item 6: a namespace member is listed at its property, which no identifier reference names, so nothing walked it.
  "use-namespace-throw": { ...USE_READ, importLines: ['import * as Nsm from "./nsm";'], files: { "nsm.ts": 'import { createSignal } from "solid-js";\nexport const [nsc] = createSignal(1);\n' }, text: (o) => `void ${o}; try { throw Nsm.nsc; } catch (e) { void (e as () => number)(); }` },
  // H5: a bare child is what its parent does with it; `For` calls it as the mapper.
  "use-child-for": { ...USE_READ, imports: ["For"], text: (o) => `const el = <For each={[1]}>{${o}}</For>; void (el as unknown as () => unknown)();` },
  // P12 (R2b's held patterns): a store a hook returns, read through the hook's call; and a `let` export of an accessor.
  "use-hook-store-member": { ...USE_READ, imports: ["createStore"], importLines: ['const [hsm] = createStore({ v: 1 });', 'const useHsm = () => hsm;'], text: (o) => `void ${o}; void useHsm().v;` },
  // The hook is App's own export and another module reads through it, so the obligation lands in App.tsx, the file the grid judge reads.
  "use-hook-store-module": { ...USE_READ, imports: ["createStore"], importLines: ['const [hse] = createStore({ v: 1 });', 'export function useHse() { return hse; }', 'import { readHse } from "./hse";'], files: { "hse.ts": 'import { useHse } from "./App";\nexport const readHse = () => useHse().v;\n' }, text: (o) => `void ${o}; void readHse();` },
  // ADR 0860: the source escapes at module scope, on a line of its own, and the site reads it in the body. The escape's row is no row
  // at the read: the old build's only SC9012 stood on the escape's line, so a judge that keeps the site saw the read clean.
  "use-map-apart": { ...USE_READ, legal: ["memo", "effect-compute"], importLines: ["const [emc] = createSignal(1);", 'const emm = new Map([["k", emc]]);'], text: (o) => `void ${o}; void emm.get("k")!();` },
  "use-logical-apart": { ...USE_READ, legal: ["memo", "effect-compute"], importLines: ["const [elc] = createSignal(1);", "const elh = ((): (() => number) | null => null)() ?? elc;"], text: (o) => `void ${o}; void elh();` },
  "use-assign-apart": { ...USE_READ, legal: ["memo", "effect-compute"], importLines: ["const [eac] = createSignal(1);", "const eao = Object.assign({}, { c: eac });"], text: (o) => `void ${o}; void eao.c();` },
  "use-each-apart": { ...USE_READ, legal: ["memo", "effect-compute"], importLines: ["const [eec] = createSignal(1);", "const eef = [{ run: (): number => 0 }];", "Object.assign(eef, [{ run: eec }]);"], text: (o) => `void ${o}; eef.forEach((f) => void f.run());` },
  "use-global-apart": { ...USE_READ, legal: ["memo", "effect-compute"], importLines: ["const [egc] = createSignal(1);", "(globalThis as any).__egc = egc;"], text: (o) => `void ${o}; void (globalThis as any).__egc();` },
  "use-untrack-apart": { ...USE_READ, legal: ["memo", "effect-compute"], imports: ["untrack"], importLines: ["const [euc] = createSignal(1);", "const euh = untrack(() => euc);"], text: (o) => `void ${o}; void euh();` },
  "use-self-apart": { ...USE_READ, legal: ["memo", "effect-compute"], importLines: ["const [esc] = createSignal(1);", "const esx = { read: (): number => 0, self: function (this: unknown) { return this; } };", "const esy: any = esx.self();", 'esy[String("read")] = esc;'], text: (o) => `void ${o}; void esx.read();` },
  "use-store-map-apart": { ...USE_READ, legal: ["memo", "effect-compute"], imports: ["createStore"], importLines: ["const [esm] = createStore({ v: 1 });", 'const esmm = new Map([["s", esm]]);'], text: (o) => `void ${o}; void esmm.get("s")!.v;` },
  "use-store-nullish-apart": { ...USE_READ, legal: ["memo", "effect-compute"], imports: ["createStore"], importLines: ["const [esn] = createStore({ v: 1 });", "const esn2 = ((): typeof esn | null => null)() ?? esn;"], text: (o) => `void ${o}; void esn2.v;` },
  "use-hook-module-apart": { ...USE_READ, legal: ["memo", "effect-compute"], importLines: ['import { useEhs } from "./ehm";'], files: { "ehm.ts": 'import { createStore } from "solid-js";\nconst [ehs] = createStore({ v: 1 });\nexport function useEhs() { return ehs; }\n' }, text: (o) => `void ${o}; void useEhs().v;` },
  // W3 (review of 7c0eff2d9): a write into a call's result (`ecb().v = ecc`), and an escaped read in a store setter's callback (run on
  // the caller's stack, though its role is a deferred callback).
  "use-call-target-apart": { ...USE_READ, imports: ["createStore"], legal: ["memo", "effect-compute"], importLines: ["const [ect] = createStore({ a: 1 });", "const ech: any = {};", "const ecb = () => ech;", "ecb().s = ect;"], text: (o) => `void ${o}; void ech.s.a;` },
  // W5 (R19a): the written call's callee is a call's result, which `function_called_at` does not follow.
  "use-call-made-apart": { ...USE_READ, imports: ["createStore"], legal: ["memo", "effect-compute"], importLines: ["const [ecm] = createStore({ a: 1 });", "const ecmh: any = {};", "const ecmk = (f: () => number) => { void f; return () => ecmh; };", "const ecmb = ecmk(() => 0);", "ecmb().s = ecm;"], text: (o) => `void ${o}; void ecmh.s.a;` },
  "use-setter-apart": { ...USE_READ, contexts: ["body"], importLines: ['import { setSw2 } from "./sw2";', "const [esp] = createSignal(1);", "const esph = ((): (() => number) | null => null)() ?? esp;"], files: SETTER_STORE.files, text: (o) => `void ${o}; setSw2((d) => { d.n = esph(); });` },
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
// ADR 0803: a slot a write to array iteration withdrew, held in another module (`slots.ts`) and used in App.tsx through the holders the
// corpus spells (an IIFE's tuple, a `let`, a `const` alias, a hook's object, an object or class instance, a prop's value). The iterator is
// written back unchanged (`iter-same`), so the use runs and Chrome warns; f11b000cd had no row at the use (corpus list m11, 35 lines).
const SLOT = "const [a, b] = createSignal(1);";
const SLOT_HOLDERS = {
  iife: { file: 'export const [r, setR] = (() => { const [r, setR] = createSignal(1); return [r, setR] as const; })();', names: ["r", "setR"], read: "r()", write: "setR(2)" },
  let: { file: `${SLOT}\nexport let r = a;\nexport let setR = b;`, names: ["r", "setR"], read: "r()", write: "setR(2)" },
  alias: { file: `${SLOT}\nexport const r = a;\nexport const setR = b;`, names: ["r", "setR"], read: "r()", write: "setR(2)" },
  hook: { file: "export function useR() {\n  const [r, setR] = createSignal(1);\n  return { r, setR };\n}", names: ["useR"], read: "useR().r()", write: "useR().setR(2)", held: "const h = useR();", heldRead: "h.r()", heldWrite: "h.setR(2)" },
  object: { file: `${SLOT}\nexport const box = { r: a, setR: b };`, names: ["box"], read: "box.r()", write: "box.setR(2)" },
  class: { file: `${SLOT}\nclass Box {\n  r = a;\n  setR = b;\n}\nexport const box = new Box();`, names: ["box"], read: "box.r()", write: "box.setR(2)" },
};
const slotHost = (holder, store = false) => ({
  kind: "plain",
  contexts: ["iter-same"],
  importLines: [`import { ${holder.names.join(", ")} } from "./slots";`],
  files: { "slots.ts": `import { ${store ? "createStore" : "createSignal"} } from "solid-js";\n\n${holder.file}\n` },
});
const SLOT_READS = Object.fromEntries(Object.entries(SLOT_HOLDERS).flatMap(([name, holder]) => [
  [`wd-${name}-read`, { ...slotHost(holder), text: () => `void ${holder.read};` }],
  ...(holder.held ? [[`wd-${name}-held-read`, { ...slotHost(holder), text: () => `${holder.held} void ${holder.heldRead};` }]] : []),
]));
// A prop whose value reads the slot, read in the child's body (its strict window): a signal's accessor, and a store's key.
const propRead = (value) => () => `const K = (p: { v: number }) => { void p.v; return null; }; void children(() => <K v={${value}} />)();`;
SLOT_READS["wd-prop-read"] = { ...slotHost(SLOT_HOLDERS.alias), imports: ["children"], text: propRead("r()") };
SLOT_READS["wd-prop-store-read"] = {
  ...slotHost({ file: "export const [st, setSt] = createStore({ v: 1 });", names: ["st"] }, true),
  imports: ["children"],
  text: propRead("st.v"),
};
const SLOT_WRITES = Object.fromEntries(Object.entries(SLOT_HOLDERS).flatMap(([name, holder]) => [
  [`wd-${name}-write`, { ...slotHost(holder), text: () => `${holder.write};` }],
  ...(holder.held ? [[`wd-${name}-held-write`, { ...slotHost(holder), text: () => `${holder.held} ${holder.heldWrite};` }]] : []),
]));

// Reviewer's W3 gap: a read past an await in an async compute, through a holder the census follows (`[n]`), was cleared by the
// compute's role.
export const AWAIT_USES = {
  "use-await-apart": { kind: "plain", contexts: ["memo-async", "effect-async"], importLines: ["const [eaw] = createSignal(1);", "const eawf = [eaw];"], text: (o) => `void ${o}; void eawf[0]();` },
};

export const FLIP_READ_VARIANTS = {
  UNTRACKED_READ_AFTER_AWAIT: AWAIT_USES,
  MISSING_EFFECT_FN: SUPPRESSED_EFFECTS,
  PENDING_ASYNC_UNTRACKED_READ: { "open-any": { kind: "open-any", contexts: ["body", "root"] }, "open-notready": { kind: "open-notready", contexts: ["body", "root"] } },
  STRICT_READ_UNTRACKED: { ...HANDED_READS, ...CREATED_READS, ...ABSENT_GUARD, ...NET_READS, ...STAR_AS_READS, ...SETTER_READS, ...VALUE_USES, ...EXPORT_COPY_READS, ...SLOT_READS },
  REACTIVE_WRITE_IN_OWNED_SCOPE: { ...NET_WRITES, ...VALUE_WRITES, ...SLOT_WRITES },
  ACTION_CALLED_IN_OWNED_SCOPE: NET_ACTIONS,
  PENDING_ASYNC_FORBIDDEN_SCOPE: EXPORT_READS,
  ASYNC_OUTSIDE_LOADING_BOUNDARY: { ...NET_PENDING, ...OPEN_COMPUTES, ...SETTLED_SLOTS, ...PENDING_SLOTS, ...PROBE_READS },
  CAUGHT_REACTIVE_READ: { ...CAUGHT_CHAINS, ...CAUGHT_HANDED_READS },
  SYNC_NODE_RECEIVED_ASYNC: SYNC_NODE_READS,
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

/** A second copy of the site's statements, each local declared under a new name. */
function freshCopy(stmts, kit) {
  const names = new Set();
  JSON.stringify(stmts, (key, value) => { if (value?.k === "decl") names.add(value.name); return value; });
  let text = JSON.stringify(stmts);
  for (const name of names) text = text.replace(new RegExp(`"${name}"`, "g"), JSON.stringify(kit.name(name.replace(/\d+$/, ""))));
  return JSON.parse(text);
}

export function flipReadContext(context, stmts, kit) {
  if (STORED_READS[context]) { const m = kit.name("m"); const made = STORED_READS[context](m, kit); return { body: [decl(m, prim("memo", fn([...stmts, ret(num(1))]))), ...(made.body ?? [])], view: made.view ?? [] }; }
  if (STORED_ASYNC[context]) { const m = kit.name("m"); const made = STORED_ASYNC[context](m, kit); return { body: [decl(m, prim("memo", fn([expr(domExpr(`${P_AWAIT};`)), ...stmts, ret(num(1))], { async: true }))), ...(made.body ?? [])], view: [] }; }
  if (STORED_SITES[context]) return STORED_SITES[context](stmts, kit);
  const stored = STORED_CONTEXTS.includes(context) && STORED_FORMS[context.replace(/-(un)?read$/, "")];
  if (stored) { const c = kit.name("c"); return { body: [decl(c, stored(stmts)), ...(context.endsWith("-read") ? [effectReading(`${c}()`)] : [])], view: [] }; }
  if (context === ASYNC_ENTRY || context === "async-entry-late") {
    const h = kit.name("f");
    const held = decl(h, fn(context === ASYNC_ENTRY ? stmts : [expr(domExpr("await 0;")), ...stmts], { async: true }));
    return { body: [held], view: [child(kit.name("v"), iife(fn([expr(domExpr(`void ${h}();`)), ret(num(1))])))] };
  }
  if (context === FLUSH_READ_STORED) {
    const keep = decl("hkSaved", domExpr("[] as (() => void)[]"));
    const twice = decl("hkTwice", domExpr("(g: () => void) => { g(); hkSaved.push(g); }"));
    return { body: [keep, twice, decl(kit.name("m"), prim("memo", fn([expr(flush(fn([ctl("run-kept", stmts)]))), ret(num(1))]))), expr(domExpr("hkSaved[0]?.();"))], view: [] };
  }
  if (COND_AWAITS[context] || HELPER_AWAITS.includes(context) || COND_AWAITS_LEGAL.includes(context) || context === "aw-wrapper-untracked" || context === "aw-inplace-stmt") {
    const watch = (m) => expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }));
    const memo = (body) => { const m = kit.name("m"); return [decl(m, prim("memo", fn([...body, ret(num(1))], { async: true }))), watch(m)]; };
    const hop = expr(domExpr(`${P_AWAIT};`));
    if (COND_AWAITS[context]) return { body: memo([expr(domExpr(COND_AWAITS[context])), ...stmts]), view: [] };
    if (context === "aw-early") return { body: memo([expr(domExpr(`if (${OFF}) { ${P_AWAIT}; return 1; }`)), ...stmts]), view: [] };
    if (COND_AWAITS_LEGAL.includes(context)) return { body: memo([ctl(context, stmts)]), view: [] };
    const h = kit.name("f");
    const awaitH = expr(domExpr(`await ${h}();`));
    const asyncFn = (body) => decl(h, fn([...body, ret(num(1))], { async: true }));
    if (context === "aw-shared-after") return { body: [asyncFn([hop, ...stmts]), ...memo([...freshCopy(stmts, kit), awaitH]), ...memo([awaitH])], view: [] };
    if (context === "aw-shared-before") return { body: [asyncFn([hop, ...stmts]), ...memo([awaitH]), ...memo([...freshCopy(stmts, kit), awaitH])], view: [] };
    if (context === "aw-helper-late") return { body: [asyncFn([...freshCopy(stmts, kit), hop, ...stmts]), ...memo([hop, awaitH])], view: [] };
    const c = kit.name("f");
    const compute = decl(c, fn([hop, ...stmts, ret(num(1))], { async: true }));
    const type = "(f: () => Promise<number>)";
    if (context === "aw-wrapper-untracked") return { body: [compute, decl(h, domExpr(`${type} => { void f(); }`)), expr(domExpr(`${h}(${c});`))], view: [] };
    if (IN_PLACE_USED.includes(context) || context === "aw-inplace-stmt") {
      const m = kit.name("m");
      const syncMemo = (body) => [decl(m, prim("memo", fn(body))), watch(m)];
      if (context === "aw-inplace-return") return { body: [compute, decl(h, domExpr(`${type} => f()`)), ...syncMemo([ret(domExpr(`${h}(${c})`))])], view: [] };
      if (context === "aw-inplace-await") return { body: [compute, decl(h, domExpr(`async ${type} => await f()`)), ...syncMemo([ret(domExpr(`${h}(${c})`))])], view: [] };
      if (context === "aw-inplace-stmt") return { body: [compute, decl(h, domExpr(`${type} => { f(); }`)), ...syncMemo([expr(domExpr(`${h}(${c});`)), ret(num(1))])], view: [] };
      const prefix = decl(c, fn([...stmts, ret(num(1))], { async: true }));
      return { body: [prefix, decl(h, domExpr(`async ${type} => { ${P_AWAIT}; f(); }`)), ...memo([expr(domExpr(`await ${h}(${c});`))])], view: [] };
    }
    const [wrap, m] = [kit.name("w"), kit.name("m")];
    const inner = decl(h, domExpr(`${type} => createMemo(f)`, undefined, ["createMemo"]));
    const read = expr(domExpr(`createEffect(() => ${m}(), () => {});`, undefined, ["createEffect"]));
    if (context === "aw-wrapper") return { body: [compute, inner, decl(m, domExpr(`${h}(${c})`)), read], view: [] };
    return { body: [compute, inner, decl(wrap, domExpr(`${type} => ${h}(f)`)), decl(m, domExpr(`${wrap}(${c})`)), read], view: [] };
  }
  if (THEN_HANDLERS[context] || context === "aw-catch-member") {
    const [h, o, m] = [kit.name("f"), kit.name("o"), kit.name("m")];
    const late = context === "aw-then-calls-async";
    const handler = decl(h, fn([...(late ? [expr(domExpr(`${P_AWAIT};`))] : []), ...stmts, ret(num(1))], { async: context === "aw-then-async" || late }));
    const holder = decl(o, domExpr(`{ fmt: ${h} }`));
    const flight = context === "aw-catch-member" ? `Promise.resolve(1).catch(${o}.fmt)` : THEN_HANDLERS[context](h, o);
    const watch = expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }));
    return { body: [handler, holder, decl(m, prim("memo", fn([ret(domExpr(flight))]))), watch], view: [] };
  }
  if (ENTRY_CHAINS[context] || context === "entry-chain-handler") {
    const [n, calls] = ENTRY_CHAINS[context] ?? [5, 2];
    const names = Array.from({ length: n }, () => kit.name("f"));
    const held = names.map((name, i) => decl(name, fn(i === 0 ? stmts : Array.from({ length: calls }, () => expr(call(ref(names[i - 1])))))));
    const entry = expr(call(ref(names[n - 1])));
    return { body: held, view: [context === "entry-chain-handler" ? button(kit.name("b"), [entry]) : child(kit.name("v"), iife(fn([entry, ret(num(1))])))] };
  }
  if (FLUSH_READ_BODY.includes(context) || FLUSH_READ_LEGAL.includes(context)) {
    const k = kit.name("f");
    const held = decl(k, fn([expr(flush(fn(context.endsWith("-iife") ? [expr(iife(fn(stmts)))] : stmts))), ret(num(1))]));
    if (FLUSH_READ_BODY.includes(context)) return { body: [held, expr(call(ref(k)))], view: [] };
    return { body: [held], view: [child(kit.name("v"), iife(fn([expr(context.startsWith("flush-read-each") ? domExpr(`[1].forEach(${k})`) : call(ref(k))), ret(num(1))])))] };
  }
  if (HANDED_FLUSH.includes(context) || HANDED_FLUSH_LEGAL.includes(context)) {
    if (context === "flush-iife") return { body: [], view: [child(kit.name("v"), iife(fn([expr(flush(fn(stmts))), ret(num(1))])))] };
    const k = kit.name("K");
    const held = decl(k, fn([expr(flush(fn(stmts))), ret(jsx())]));
    const entry = { "handed-flush-each": `[1].forEach(${k})`, "handed-flush-call": `${k}.call(null)`, "named-flush-call": `${k}()`, "handed-flush-handler": `[1].map(${k})`, "handed-flush-empty": `([] as number[]).map(${k})` }[context];
    if (context === "handed-flush-tag") return { body: [held], view: [mounted(kit.name("v"), domExpr(`<${k} />`))] };
    if (context === "handed-flush-handler") return { body: [held], view: [button(kit.name("b"), [expr(domExpr(entry))])] };
    return { body: [held], view: [child(kit.name("v"), iife(fn([expr(domExpr(entry)), ret(num(1))])))] };
  }
  // ADR 0835 review R1/R6: the helper `u` holds the site; one call of it is placed (tracked, under `<Loading>`), the other path is cut
  // (a chain of five past the walk's bound in `onSettled`, a guarded effect apply, a handler).
  if (context === "export-placed" || context === "export-placed-l") {
    const placed = mounted(kit.name("v"), domExpr('<Loading fallback={0}><i>{(eRead(), "x")}</i></Loading>', undefined, ["Loading"]));
    return { hoisted: [decl("eRead", fn(stmts)), expr(domExpr("export { eRead }"))], body: [], view: [placed, mounted(kit.name("w"), domExpr("<EO />"))] };
  }
  if (["settled-chain-placed", "apply-placed-twin", "handler-chain-placed"].includes(context)) {
    const u = kit.name("u");
    const hc = Array.from({ length: 5 }, () => kit.name("hc"));
    const chain = hc.map((name, i) => decl(name, fn([expr(call(ref(i === 0 ? u : hc[i - 1])))])));
    const placed = mounted(kit.name("v"), domExpr(`<Loading fallback={0}><i>{(${u}(), "x")}</i></Loading>`, undefined, ["Loading"]));
    const other = {
      "settled-chain-placed": [...chain, expr(prim("settled", fn([expr(call(ref(hc[4])))])))],
      "apply-placed-twin": [expr(prim("effect", fn([ret(num(1))]), { apply: fn([expr(call(ref(u)))]) }))],
      "handler-chain-placed": chain,
    }[context];
    const view = context === "handler-chain-placed" ? [button(kit.name("b"), [expr(call(ref(hc[4])))]), placed] : [placed];
    return { body: [decl(u, fn(stmts)), ...other], view };
  }
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
  const cast = CASTS[context] ?? RECEIVERS[context] ?? LATE_RECEIVERS[context];
  if (cast) {
    const { hoisted = [], held, callee, call: run, files, importLines } = cast(kit.name("t"), kit.name("q"));
    const h = kit.name("f");
    return { hoisted, body: [...held, decl(h, fn(stmts)), expr(domExpr(run ? run(h) : `${callee}(${h})`))], view: [], ...(files ? { files, importLines } : {}) };
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
  const iterPatch = ITER_PATCHES[context] ?? ITER_SAME[context];
  if (iterPatch) return { top: [expr(domExpr(iterPatch))], body: stmts, view: [] };
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
  // ADR 0874, review R30-R32: `names` is the program's own object, typed as a list: its `forEach` runs nothing on this stack.
  if (context === "wh-helper-late" || context === "wh-action-late" || context === "wh-root-cleanup") {
    const [own, names, f, h] = [kit.name("o"), kit.name("n"), kit.name("f"), kit.name("h")];
    const each = context === "wh-root-cleanup" ? "(cb: () => void) => { onCleanup(cb); }" : "(cb: () => void) => { queueMicrotask(cb); }";
    const hold = expr(domExpr(`const ${own}: any = {}; ${own}.forEach = ${each}; const ${names}: string[] = ${own};`, undefined, context === "wh-root-cleanup" ? ["onCleanup"] : undefined));
    const site = decl(f, fn(stmts));
    if (context === "wh-helper-late") return { body: [hold, site, decl(h, domExpr(`() => { ${names}.forEach(() => { ${f}(); }); }`)), expr(domExpr(`${h}()`))], view: [] };
    if (context === "wh-action-late") return { body: [hold, site, decl(h, domExpr(`action(function* () { ${names}.forEach(() => { ${f}(); }); yield; })`, undefined, ["action"])), expr(domExpr(`${h}()`))], view: [] };
    return { body: [hold, site, expr(domExpr(`${names}.forEach(() => { createRoot(() => { ${f}(); }); })`, undefined, ["createRoot"]))], view: [] };
  }
  // ADR 0874: `then` on a receiver no fact proves, under a key no fact names: the read past the await keeps its rule's row. Chrome warns there.
  if (context === "then-withheld" || context === "then-withheld-named") {
    const [t, f, m, q] = [kit.name("t"), kit.name("f"), kit.name("m"), kit.name("q")];
    const star = expr(domExpr(`const ${t}: unknown[] = []; ${t}[Number("0")] = () => 0;`));
    const own = decl(q, domExpr("(x: Promise<number>) => x"));
    // Raw text, as `then-named`: the validator types no member of a call's result. The inline form keeps an arrow at the call.
    const named = context === "then-withheld-named";
    const flight = domExpr(`${q}(Promise.resolve(1)).then(${named ? f : `() => ${f}()`})`);
    const compute = fn([awaitStmt(), ret(flight)], { async: true });
    return { hoisted: [star], body: [own, decl(f, fn([...stmts, ret(num(1))])), decl(m, prim("memo", compute)), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))], view: [] };
  }
  // ADR 0874: R10e, a `then` the program put on Object.prototype makes the awaited array thenable: `await` hands back its object, whose forEach never runs the callback.
  if (context === "await-object-then") {
    const [t, f, m] = [kit.name("t"), kit.name("f"), kit.name("m")];
    const patch = 'Object.defineProperty(Object.prototype, "then", { configurable: true, get(this: any) { return Array.isArray(this) && this[0] === 7 ? (r: any) => r({ forEach() {} }) : undefined; } });';
    return { hoisted: [expr(domExpr(patch))], body: [decl(f, fn(stmts)), decl(m, prim("memo", fn([decl(t, domExpr("await [7]")), expr(domExpr(`${t}.forEach(${f})`)), ret(num(1))], { async: true }))), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))], view: [] };
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
