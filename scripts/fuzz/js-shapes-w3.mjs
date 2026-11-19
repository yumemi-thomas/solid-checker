// ADR 1117, W3's half of the JS-semantics axis (js-shapes.mjs holds W4's cells and the context that places both): the same site in `jo.f`, reached after an async memo's
// await by a route through a patched prototype, event delivery, a shadow root, a key the checker must compute, or a module re-export. Seeds: the ADR 1108 part 2 probes
// (proto-site-*, global-route-*, scripts/fuzz/open-defects.json `pkgaction/patch-f32-undeclared` and `pkgaction/global-route-event-view`, written there in a package and
// for ACTION_CALLED_IN_OWNED_SCOPE; here in the project, for the site after an await).
//
//   Patched standard and DOM prototypes
//   js-proto-tojson     `Date.prototype.toJSON = jo.f`, run by `JSON.stringify` (an implicit library call, no call to the method written)
//   js-proto-reverse    `Float32Array.prototype.reverse = jo.f`, run by a written `.reverse()` the typings declare as the built-in
//   js-proto-exec       `RegExp.prototype.exec` wrapped, run by `/fz/.test("x")` (the library calls `exec`)
//   js-proto-chain      a getter on an object set as another's prototype (`Object.setPrototypeOf`), read through the child
//   js-proto-ael        `EventTarget.prototype.addEventListener` wrapped, run by a written `window.addEventListener`
//   EventTarget
//   js-evt-dispatch     a listener on a `new EventTarget()`, run by `dispatchEvent`
//   js-evt-window       a listener on the window by the bare global `addEventListener`, run by the bare global `dispatchEvent` (`this` is the window)
//   js-evt-handle       a listener object (`{ handleEvent: jo.f }`): delivery calls the method
//   js-evt-this         a listener that calls through `this` (the window), the hook stored on the window
//   js-global-alias     the hook stored through `globalThis`, called through `window` (one object, two names)
//   Shadow DOM
//   js-shadow-listen    a listener on a shadow root, run by `host.shadowRoot.dispatchEvent`
//   js-shadow-composed  a listener on the host, run by a composed event dispatched inside its shadow root
//   js-shadow-getter    a getter defined on the shadow root, read through `host.shadowRoot`
//   Escaped and concatenated keys
//   js-key-concat       `jr["se" + "t"]()`
//   js-key-template     `` jr[`set`]() ``
//   js-key-escaped      `jr["s\x65t"]()`
//   js-key-unicode      `{ set: jo.f }`, called as `jr.set()`
//   js-key-unicode-call `{ set: jo.f }`, called as `jr.set()`
//   Re-exports: a module-scope helper `jx(o) { return o.f(); }` exported by App and imported back through a second module, called with `jo`
//   js-reexp-default    `export { jx as default } from "./App"`
//   js-reexp-star       `export * from "./App"`
//   js-reexp-ns         `export * as JN from "./App"`, called as `JN.jx(jo)`
//   js-reexp-barrel     `export { jx as jm } from "./App"` in one module, `export * from` it in a barrel

/** The helper the re-export cells reach the hook by: module scope, exported, it calls the member it is handed. */
const helper = (n) => [`export function ${n}x(o: { f: () => unknown }) { return o.f(); }`];
const reexport = (files, importLine, callee) => ({
  extra: (n) => ({ hoisted: helper(n), importLines: [importLine(n)], files: files(n) }),
  run: (n) => `void ${callee(n)}({ f: ${n}h })`,
});
const reexportRoute = ({ files, importLine, callee }) => {
  const { extra, run } = reexport(files, importLine, callee);
  return ["null", (f, n) => [`const ${n}h = ${f}`], run, extra];
};

export const W3_ROUTES = {
  "js-proto-tojson": ['""', (f) => [`Date.prototype.toJSON = ${f} as unknown as Date["toJSON"]`], () => "void JSON.stringify({ d: new Date(0) })"],
  "js-proto-reverse": ["0", (f) => [`(Float32Array.prototype as unknown as { reverse: unknown }).reverse = ${f}`], () => "void new Float32Array(1).reverse()"],
  "js-proto-exec": ["null", (f, n) => [`const ${n}x = RegExp.prototype.exec`, `RegExp.prototype.exec = function (this: RegExp, s: string) { if (this.source === "fz") void ${f}(); return ${n}x.call(this, s); }`], () => 'void /fz/.test("x")'],
  "js-proto-chain": ["1", (f, n) => [`const ${n} = Object.setPrototypeOf({}, Object.defineProperty({}, "v", { get: ${f} })) as { v?: number }`], (n) => `void ${n}.v`],
  "js-proto-ael": ["null", (f, n) => [`const ${n}x = EventTarget.prototype.addEventListener`, `EventTarget.prototype.addEventListener = function (this: EventTarget, type: string, l: EventListenerOrEventListenerObject | null, o?: boolean | AddEventListenerOptions) { if (type === "fz") void ${f}(); ${n}x.call(this, type, l, o); }`], () => 'window.addEventListener("fz", () => {})'],
  "js-evt-dispatch": ["null", (f, n) => [`const ${n} = new EventTarget()`, `${n}.addEventListener("fz", ${f})`], (n) => `void ${n}.dispatchEvent(new Event("fz"))`],
  "js-evt-window": ["null", (f) => [`addEventListener("fz", ${f})`], () => 'void dispatchEvent(new Event("fz"))'],
  "js-evt-handle": ["null", (f) => [`window.addEventListener("fz", { handleEvent: ${f} })`], () => 'void window.dispatchEvent(new Event("fz"))'],
  "js-evt-this": ["null", (f) => [`(window as Window & { jw?: () => unknown }).jw = ${f}`, 'window.addEventListener("fz", function (this: Window & { jw?: () => unknown }) { void this.jw?.(); })'], () => 'void window.dispatchEvent(new Event("fz"))'],
  "js-global-alias": ["null", (f) => [`(globalThis as unknown as { jw: () => unknown }).jw = ${f}`], () => "void (window as unknown as { jw: () => unknown }).jw()"],
  "js-shadow-listen": ["null", (f, n) => [`const ${n} = document.createElement("div")`, `${n}.attachShadow({ mode: "open" }).addEventListener("fz", ${f})`], (n) => `void ${n}.shadowRoot?.dispatchEvent(new Event("fz"))`],
  "js-shadow-composed": ["null", (f, n) => [`const ${n} = document.createElement("div")`, `${n}.addEventListener("fz", ${f})`, `${n}.attachShadow({ mode: "open" }).append(document.createElement("i"))`], (n) => `void ${n}.shadowRoot?.firstElementChild?.dispatchEvent(new Event("fz", { bubbles: true, composed: true }))`],
  "js-shadow-getter": ["1", (f, n) => [`const ${n} = document.createElement("div")`, `Object.defineProperty(${n}.attachShadow({ mode: "open" }), "v", { get: ${f} })`], (n) => `void (${n}.shadowRoot as unknown as { v: number }).v`],
  "js-key-concat": ["null", (f, n) => [`const ${n} = { set: ${f} }`], (n) => `void ${n}[("se" + "t") as "set"]()`],
  "js-key-template": ["null", (f, n) => [`const ${n} = { set: ${f} }`], (n) => `void ${n}[\`set\`]()`],
  "js-key-escaped": ["null", (f, n) => [`const ${n} = { set: ${f} }`], (n) => `void ${n}["s\\x65t"]()`],
  "js-key-unicode": ["null", (f, n) => [`const ${n} = { s\\u0065t: ${f} }`], (n) => `void ${n}.set()`],
  "js-key-unicode-call": ["null", (f, n) => [`const ${n} = { set: ${f} }`], (n) => `void ${n}.s\\u0065t()`],
  "js-reexp-default": reexportRoute({ files: (n) => ({ "jlib.ts": `export { ${n}x as default } from "./App";\n` }), importLine: (n) => `import ${n}d from "./jlib";`, callee: (n) => `${n}d` }),
  "js-reexp-star": reexportRoute({ files: () => ({ "jlib.ts": 'export * from "./App";\n' }), importLine: (n) => `import { ${n}x as ${n}l } from "./jlib";`, callee: (n) => `${n}l` }),
  "js-reexp-ns": reexportRoute({ files: () => ({ "jlib.ts": 'export * as JN from "./App";\n' }), importLine: () => 'import { JN } from "./jlib";', callee: (n) => `JN.${n}x` }),
  "js-reexp-barrel": reexportRoute({ files: (n) => ({ "jmid.ts": `export { ${n}x as ${n}m } from "./App";\n`, "jlib.ts": 'export * from "./jmid";\n' }), importLine: (n) => `import { ${n}m } from "./jlib";`, callee: (n) => `${n}m` }),
};
