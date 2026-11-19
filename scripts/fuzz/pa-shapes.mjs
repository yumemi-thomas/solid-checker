// ADR 0972: a call into code of a package the build does not read as source, written where an owner may be active. The package may call Solid's `action` itself,
// so the call throws ACTION_CALLED_IN_OWNED_SCOPE although the program names no action there. A cell crosses what the package is with where the call is written:
//
//   pa-<package>-<host>
//     packages  reach    `fz-act`: its `paEnter()` calls `action(function* () {})()`; its manifest names no Solid package, so the build does not read it
//               pure     `fz-calm`: names no Solid package in its closure, loads nothing by a computed name, reads only the libraries' globals (the checker clears it)
//               dyn      `fz-load`: loads `import(name)` on a branch that never runs (the checker cannot clear it; Chrome is quiet)
//               inline   no package: the program's own `action(function* () {})()`, an action called where it is created (the second hole of ADR 0972)
//               winself  `fz-pub` (a side-effect import that reaches Solid and writes `window.__paAct`) and `fz-winself`, whose `paEnter()` calls `window.self.__paAct()`:
//                        the global object through its own member (ADR 0974: the scan followed one member and cleared it)
//               docview  `fz-pub` and `fz-docview`, whose `paEnter()` calls `document.defaultView.__paAct()` (ADR 0974)
//               getter   `fz-getter`: `clock` has `get now()` that calls the action; the site is the read `void clock.now`, no call (ADR 0974)
//               gwrite   `fz-gwrite` replaces the host's `btoa` (`globalThis.btoa = …`) with a function that calls the action; the site is `btoa("x")`, a global the library
//                        declares, so the compiler's signature is the host's (ADR 0974: the write comes from the scan of the package's closure)
//               gwritekey  `fz-gwritekey`: the same, the key a `const KEY = "btoa"` (the file's constants decide it)
//               gwriteany  `fz-gwriteany`: the same through `Object.assign(globalThis, loadPatches())`: a key nothing in the file decides, so any global may be written
//               gwritesym  `fz-gwritesym` reaches Solid and writes only a symbol-keyed property: no global a call can run, `btoa("x")` is the host's (legal)
//               gwriteand  `fz-gwriteand`: `globalThis.btoa &&= …` (a logical assignment writes as `=` does, ADR 0974)
//               gwritereflect  `fz-gwritereflect`: `Reflect.apply(Object.assign, null, [globalThis, { btoa }])` (a writer called through `Reflect.apply`, ADR 0974)
//               gwritealias  `fz-gwritealias`: `const assign = Object.assign; assign(globalThis, { btoa })` (a writer held by a name, ADR 0974)
//               gwritebind   `fz-gwritebind`: `Object.assign.bind(null, globalThis)` called with the patch (a bound writer, ADR 0974)
//               gcallkey, gcallconst, gcalldestr, gcallalias, gcallarray, gcallself, gcallhanded  `fz-gwrite` again, the program reaching the replaced `btoa` by another
//                        route to the global object: `window["btoa"]("x")`, a constant key, `const { btoa: enc } = window`, `const enc = window.btoa`, `[btoa][0]("x")`,
//                        `window.self.btoa("x")`, the global object handed to a function that calls it (ADR 0974: every route the call side can decide is followed, and the
//                        object handed on, where any global may be read, is a hand-off; the site is the call)
//               getterrename, getterdefault  `fz-getter` again, the getter read through a pattern: `({ now: n } = clock)`, `({ now = 0 } = clock)` (ADR 0974: the row stands at the
//                        pattern's property, where the runtime reports the call, not at `clock`)
//               gproto   `fz-gproto` replaces `Float32Array.prototype.reverse` with a function that calls the action; the site is `new Float32Array(1).reverse()`,
//                        a method the library declares (ADR 1108: a built-in's prototype a package writes)
//               gview    `fz-gview` replaces `btoa` through a click event's `view`, dispatched where the module loads; the site is `btoa("x")` (ADR 1108: a member
//                        the DOM types as this realm's window is the global object, as `defaultView` is; a message's `source` needs a message the browser
//                        delivers later or a `source` the program spells, an escape already, so it is the fixture's, not a kind here)
//               gprotocall, gprotoapply, gprotoreflect, gprotodestr  `fz-gproto` again, the replaced `reverse` run another way: `a.reverse.call(a)`, `a.reverse.apply(a, [])`,
//                        `Reflect.apply(a.reverse, a, [])`, `const { reverse } = a; reverse()` (ADR 1108 part 2: every call that may run a written name)
//               gprotoimplicit  `fz-gprotoexec` replaces `RegExp.prototype.exec`; the site is `/fzprobe/.test("x")`, which the library runs `exec` for (ADR 1108 part 2)
//               gprotoiter  `fz-gprotoiter` replaces `Float32Array.prototype[Symbol.iterator]` (a key the scan does not decide: any name); the site is the spread
//                        `[...new Float32Array(1)]` (ADR 1114 family 3: an iteration runs the iterator)
//               gprotoany  `fz-gprotokey` replaces `Float32Array.prototype.reverse` under a key the scan does not decide (a call's result: any name); the site is
//                        `(new Float32Array(1) as any).reverse()`, a member call no declaration resolves (ADR 1114 family 3, kind any-untyped-member)
//               gprotocast, gprotoannot  `fz-gprotokey` again; the receiver is typed by a type literal, so the call resolves to the project's declaration:
//                        `(new Float32Array(1) as unknown as { reverse(): unknown }).reverse()`, `const a: { reverse(): unknown } = new Float32Array(1); a.reverse()`
//                        (ADR 1114 family 3 slice E)
//               gprotoconcat, gprototemplate, gprotocatch, gprotobare, gprotoget, gprotoset, gprotourl  the other operations that run a written member
//                        (ADR 1114 family 3 slice E): `"" + new Date(0)` and a template's substitution after `fz-gprotodate` writes `Date.prototype.toString`;
//                        `Promise.resolve(1).catch(…)` after `fz-gprotothen` writes `then`; a bare `dispatchEvent(…)` after `fz-gprotoevent`; an accessor read
//                        after `fz-gprotoget` defines `DataView.prototype.byteLength`; `new Set(it)` after `fz-gprotoiter`; `new URL(u)` after `fz-gprotourl`
//               gviewpattern  `fz-gviewpattern` replaces `btoa` through a click event's `view` read by a pattern (`({ view }) => …`); the site is `btoa("x")` (ADR 1108 part 2)
//               gaccmember, gaccbare, gacctypeof, gaccpattern  `fz-gaccessor` redefines `devicePixelRatio` with a getter that calls the action; the site is the value read, no
//                        call: `void window.devicePixelRatio`, `void devicePixelRatio`, `void typeof window.devicePixelRatio`, `({ devicePixelRatio: ratio } = window)` (ADR 0974)
//     hosts     body     the component's body                         owned
//               memo     a memo's compute, read by the view           owned
//               root     a `createRoot` body in the component's body  owned
//               timer    a `setTimeout` callback                      legal
//               micro    a `queueMicrotask` callback                  legal
//
// A cell is misuse where the call throws (reach and inline, on an owned host) and legal otherwise. The site is the call itself (`paUse`: the code's `use` node writes the
// package's entry or the inline action), so the runtime reports the code at the site's line. Recall contexts, the direct binding bare
// (`paShape`), not crossed by the census, ACTION_CALLED_IN_OWNED_SCOPE only (not mirrored to ON_FLUSH_IN_OWNED_SCOPE: the package calls `action`, not `onFlush`).
import { call, child, decl, expr, fn, num, prim, ref, ret } from "./model.mjs";

/** The writers a package replaces `btoa` with (ADR 0974: the forms of a write the scan follows), and the program's routes to the replaced global and to a redefined accessor. */
const WRITER_KINDS = ["gwrite", "gwritekey", "gwriteany", "gwritesym", "gwriteand", "gwritereflect", "gwritealias", "gwritebind"];
const CALL_KINDS = ["gcallkey", "gcallconst", "gcalldestr", "gcallalias", "gcallarray", "gcallself", "gcallhanded"];
const ACCESSOR_KINDS = ["gaccmember", "gaccbare", "gacctypeof", "gaccpattern"];
const PATTERN_KINDS = ["getterrename", "getterdefault"];
/** ADR 1108: a built-in's prototype written, and a global written through a member the DOM types as the window. */
const ROUTE_KINDS = ["gproto", "gview"];
/** ADR 1108 part 2: the written method run by `.call`, `.apply`, `Reflect.apply`, a destructured binding, the library's own `exec`, an iteration; `view` read by a pattern. */
const PROTO_ROUTES = {
  gprotocall: { prelude: ["const a = new Float32Array(1);"], text: "a.reverse.call(a)" },
  gprotoapply: { prelude: ["const a = new Float32Array(1);"], text: "a.reverse.apply(a, [])" },
  gprotoreflect: { prelude: ["const a = new Float32Array(1);"], text: "Reflect.apply(a.reverse, a, [])" },
  gprotodestr: { prelude: ["const { reverse } = new Float32Array(1);"], text: "reverse()" },
  gprotoimplicit: { text: '/fzprobe/.test("x")' },
  gprotoiter: { text: "void [...new Float32Array(1)]" },
  gprotoany: { text: "void (new Float32Array(1) as any).reverse()" },
  gprotocast: { text: "void (new Float32Array(1) as unknown as { reverse(): unknown }).reverse()" },
  gprotoannot: { prelude: ["const a: { reverse(): unknown } = new Float32Array(1);"], text: "void a.reverse()" },
  gprotoconcat: { text: 'void ("" + new Date(0))' },
  gprototemplate: { text: "void `${new Date(0)}`" },
  gprotocatch: { text: "void Promise.resolve(1).catch(() => {})" },
  gprotobare: { text: 'void dispatchEvent(new Event("fzprobe"))' },
  gprotoget: { text: "void new DataView(new ArrayBuffer(1)).byteLength" },
  gprotoset: { text: "void new Set(new Float32Array(1))" },
  gprotourl: { text: 'void new URL(new URL("http://fz.test/"))' },
  gviewpattern: { text: 'btoa("x")' },
};
const PROTO_KINDS = Object.keys(PROTO_ROUTES);
export const PA_PACKAGE_KINDS = ["reach", "pure", "dyn", "inline", "winself", "docview", "getter", ...WRITER_KINDS, ...CALL_KINDS, ...ACCESSOR_KINDS, ...PATTERN_KINDS, ...ROUTE_KINDS, ...PROTO_KINDS];
export const PA_HOSTS = ["body", "memo", "root", "timer", "micro"];
const OWNED_HOSTS = ["body", "memo", "root"];
const THROWING_KINDS = ["reach", "inline", "winself", "docview", "getter", ...WRITER_KINDS.filter((kind) => kind !== "gwritesym"), ...CALL_KINDS, ...ACCESSOR_KINDS, ...PATTERN_KINDS, ...ROUTE_KINDS, ...PROTO_KINDS];
export const PA_CODE = "ACTION_CALLED_IN_OWNED_SCOPE";
/** The packages each kind imports from, in the order its site writes them (`inline` imports none); the last holds the entry. */
export const PA_PACKAGES = {
  reach: ["fz-act"], pure: ["fz-calm"], dyn: ["fz-load"], winself: ["fz-pub", "fz-winself"], docview: ["fz-pub", "fz-docview"], getter: ["fz-getter"],
  gwrite: ["fz-gwrite"], gwritekey: ["fz-gwritekey"], gwriteany: ["fz-gwriteany"], gwritesym: ["fz-gwritesym"],
  gwriteand: ["fz-gwriteand"], gwritereflect: ["fz-gwritereflect"], gwritealias: ["fz-gwritealias"], gwritebind: ["fz-gwritebind"],
  ...Object.fromEntries(CALL_KINDS.map((kind) => [kind, ["fz-gwrite"]])),
  ...Object.fromEntries(ACCESSOR_KINDS.map((kind) => [kind, ["fz-gaccessor"]])),
  ...Object.fromEntries(PATTERN_KINDS.map((kind) => [kind, ["fz-getter"]])),
  gproto: ["fz-gproto"], gview: ["fz-gview"],
  gprotocall: ["fz-gproto"], gprotoapply: ["fz-gproto"], gprotoreflect: ["fz-gproto"], gprotodestr: ["fz-gproto"],
  gprotoimplicit: ["fz-gprotoexec"], gprotoiter: ["fz-gprotoiter"], gprotoany: ["fz-gprotokey"], gprotocast: ["fz-gprotokey"], gprotoannot: ["fz-gprotokey"], gprotoconcat: ["fz-gprotodate"], gprototemplate: ["fz-gprotodate"], gprotocatch: ["fz-gprotothen"], gprotobare: ["fz-gprotoevent"], gprotoget: ["fz-gprotoget"], gprotoset: ["fz-gprotoiter"], gprotourl: ["fz-gprotourl"], gviewpattern: ["fz-gviewpattern"],
};
/** `{ [context]: "misuse" | "legal" }`. */
export const PA_CONTEXTS = Object.fromEntries(
  PA_PACKAGE_KINDS.flatMap((kind) => PA_HOSTS.map((host) => [`pa-${kind}-${host}`, THROWING_KINDS.includes(kind) && OWNED_HOSTS.includes(host) ? "misuse" : "legal"])),
);
export const PA_CONTEXT_NAMES = Object.keys(PA_CONTEXTS);

const NAME = new RegExp(`^pa-(${PA_PACKAGE_KINDS.join("|")})-(${PA_HOSTS.join("|")})$`);
/** `{ kind, host }` of a `pa-*` context name, or null. */
export const paParts = (context) => {
  const match = NAME.exec(context);
  return match ? { kind: match[1], host: match[2] } : null;
};

/** The base shapes a `pa-*` cell takes: the direct binding, no layer. */
export function paShape(context, shape) {
  return !paParts(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/** The files of one package of a kind (`name` is one of `PA_PACKAGES[kind]`): a manifest that names no Solid package (the build does not read its source; the program's own `solid-js` serves the import at run time). */
export function paPackageFiles(kind, name = PA_PACKAGES[kind].at(-1)) {
  const entry = ["export declare const paEnter: () => number;"];
  const files = {
    "fz-act": { js: ['import { action } from "solid-js";', "", "export const paEnter = () => {", "  void action(function* () {})();", "  return 1;", "};"], dts: entry },
    "fz-calm": { js: ["const twice = (n) => n * 2;", "", "export const paEnter = () => twice([1].length) + Math.max(1, 2);"], dts: entry },
    "fz-load": { js: ["export const paEnter = () => {", '  const name = ["./", "never.js"].join("");', '  if (name === "") void import(name);', "  return 1;", "};"], dts: entry },
    // A side-effect import that reaches Solid and writes a global; the packages that read it name no Solid package themselves.
    "fz-pub": { js: ['import { action } from "solid-js";', "", "window.__paAct = () => {", "  void action(function* () {})();", "  return 1;", "};"], dts: ["export {};"] },
    "fz-winself": { js: ["export const paEnter = () => window.self.__paAct();"], dts: entry },
    "fz-docview": { js: ["export const paEnter = () => document.defaultView.__paAct();"], dts: entry },
    "fz-getter": { js: ['import { action } from "solid-js";', "", "export const clock = {", "  get now() {", "    void action(function* () {})();", "    return 1;", "  },", "};"], dts: ["export declare const clock: { readonly now: number };"] },
    // ADR 0974: a package that replaces a global the library declares, imported for its effect: the type is sound for the declaration, the write comes from the scan.
    "fz-gwrite": { js: ['import { action } from "solid-js";', "", "const real = globalThis.btoa;", "globalThis.btoa = (text) => {", "  void action(function* () {})();", "  return real(text);", "};"], dts: ["export {};"] },
    "fz-gwritekey": { js: ['import { action } from "solid-js";', "", 'const KEY = "btoa";', "const real = globalThis[KEY];", "globalThis[KEY] = (text) => {", "  void action(function* () {})();", "  return real(text);", "};"], dts: ["export {};"] },
    "fz-gwriteany": { js: ['import { action } from "solid-js";', "", "const real = globalThis.btoa;", "function loadPatches() {", "  return {", "    btoa: (text) => {", "      void action(function* () {})();", "      return real(text);", "    },", "  };", "}", "Object.assign(globalThis, loadPatches());"], dts: ["export {};"] },
    // ADR 0974: the other forms of a write to `btoa`, each a write the scan must see: a logical assignment, a writer called through `Reflect.apply`, a writer held by a name, a bound writer.
    "fz-gwriteand": { js: ['import { action } from "solid-js";', "", "const real = globalThis.btoa;", "globalThis.btoa &&= (text) => {", "  void action(function* () {})();", "  return real(text);", "};"], dts: ["export {};"] },
    "fz-gwritereflect": { js: ['import { action } from "solid-js";', "", "const real = globalThis.btoa;", "Reflect.apply(Object.assign, null, [globalThis, {", "  btoa: (text) => {", "    void action(function* () {})();", "    return real(text);", "  },", "}]);"], dts: ["export {};"] },
    "fz-gwritealias": { js: ['import { action } from "solid-js";', "", "const real = globalThis.btoa;", "const assign = Object.assign;", "assign(globalThis, {", "  btoa: (text) => {", "    void action(function* () {})();", "    return real(text);", "  },", "});"], dts: ["export {};"] },
    "fz-gwritebind": { js: ['import { action } from "solid-js";', "", "const real = globalThis.btoa;", "const patch = Object.assign.bind(null, globalThis);", "patch({", "  btoa: (text) => {", "    void action(function* () {})();", "    return real(text);", "  },", "});"], dts: ["export {};"] },
    // ADR 0974: a global redefined with a getter: the read runs the package's code, so the site is the read, not a call.
    "fz-gaccessor": { js: ['import { action } from "solid-js";', "", 'Object.defineProperty(window, "devicePixelRatio", {', "  configurable: true,", "  get() {", "    void action(function* () {})();", "    return 1;", "  },", "});"], dts: ["export {};"] },
    // ADR 1108: a method replaced on a built-in's prototype, and `btoa` replaced through an event's `view`.
    "fz-gproto": { js: ['import { action } from "solid-js";', "", "const real = Float32Array.prototype.reverse;", "Float32Array.prototype.reverse = function () {", "  void action(function* () {})();", "  return real.call(this);", "};"], dts: ["export {};"] },
    "fz-gview": { js: ['import { action } from "solid-js";', "", "const real = globalThis.btoa;", 'document.addEventListener("click", (event) => {', "  event.view.btoa = (text) => {", "    void action(function* () {})();", "    return real(text);", "  };", "}, { once: true });", "document.documentElement.click();"], dts: ["export {};"] },
    // ADR 1108 part 2: `exec` replaced (the library runs it for `test`), and `btoa` replaced through `view` read by a pattern.
    "fz-gprotoexec": { js: ['import { action } from "solid-js";', "", "const real = RegExp.prototype.exec;", "RegExp.prototype.exec = function (text) {", '  if (this.source === "fzprobe") void action(function* () {})();', "  return real.call(this, text);", "};"], dts: ["export {};"] },
    "fz-gprotoiter": { js: ['import { action } from "solid-js";', "", "const real = Float32Array.prototype[Symbol.iterator];", "Float32Array.prototype[Symbol.iterator] = function () {", "  void action(function* () {})();", "  return real.call(this);", "};"], dts: ["export {};"] },
    "fz-gprotokey": { js: ['import { action } from "solid-js";', "", 'const key = ["re", "verse"].join("");', "const real = Float32Array.prototype[key];", "Float32Array.prototype[key] = function () {", "  void action(function* () {})();", "  return real.call(this);", "};"], dts: ["export {};"] },
    "fz-gprotodate": { js: ['import { action } from "solid-js";', "", "const real = Date.prototype.toString;", "Date.prototype.toString = function () {", "  void action(function* () {})();", "  return real.call(this);", "};"], dts: ["export {};"] },
    "fz-gprotothen": { js: ['import { action } from "solid-js";', "", "const real = Promise.prototype.then;", "Promise.prototype.then = function (...args) {", "  if (!globalThis.__fzInThen) { globalThis.__fzInThen = true; try { void action(function* () {})(); } finally { globalThis.__fzInThen = false; } }", "  return real.apply(this, args);", "};"], dts: ["export {};"] },
    "fz-gprotoevent": { js: ['import { action } from "solid-js";', "", "const real = EventTarget.prototype.dispatchEvent;", "EventTarget.prototype.dispatchEvent = function (event) {", "  if (event.type === \"fzprobe\") void action(function* () {})();", "  return real.call(this, event);", "};"], dts: ["export {};"] },
    "fz-gprotoget": { js: ['import { action } from "solid-js";', "", "const real = Object.getOwnPropertyDescriptor(DataView.prototype, \"byteLength\").get;", "Object.defineProperty(DataView.prototype, \"byteLength\", { configurable: true, get() {", "  void action(function* () {})();", "  return real.call(this);", "} });"], dts: ["export {};"] },
    "fz-gprotourl": { js: ['import { action } from "solid-js";', "", "const real = URL.prototype.toString;", "URL.prototype.toString = function () {", "  void action(function* () {})();", "  return real.call(this);", "};"], dts: ["export {};"] },
    "fz-gviewpattern": { js: ['import { action } from "solid-js";', "", "const real = globalThis.btoa;", 'document.addEventListener("click", ({ view }) => {', "  view.btoa = (text) => {", "    void action(function* () {})();", "    return real(text);", "  };", "}, { once: true });", "document.documentElement.click();"], dts: ["export {};"] },
    "fz-gwritesym": { js: ['import { action } from "solid-js";', "", 'var MARK = /* @__PURE__ */ Symbol.for("fz-gwritesym");', "globalThis[MARK] = () => {", "  void action(function* () {})();", "};"], dts: ["export {};"] },
  }[name];
  return {
    "package.json": `${JSON.stringify({ name, version: "1.0.0", type: "module", exports: { ".": { types: "./index.d.ts", default: "./index.js" } } }, null, 2)}\n`,
    "index.js": `${files.js.join("\n")}\n`,
    "index.d.ts": `${files.dts.join("\n")}\n`,
  };
}

/** ADR 0974: the program's routes to the global `fz-gwrite` replaced: `{ text, prelude? }` (the prelude is a declaration at the top of the component's body, the text the site). */
const CALL_ROUTES = {
  gcallkey: { text: 'window["btoa"]("x")' },
  gcallconst: { prelude: ['const key = "btoa";'], text: 'window[key]("x")' },
  gcalldestr: { prelude: ["const { btoa: enc } = window;"], text: 'enc("x")' },
  gcallalias: { prelude: ["const enc = window.btoa;"], text: 'enc("x")' },
  gcallarray: { text: '[btoa][0]("x")' },
  gcallself: { text: 'window.self.btoa("x")' },
  gcallhanded: { text: '((w: Window) => w.btoa("x"))(window)' },
};
/** ADR 0974: the reads of `devicePixelRatio`, which `fz-gaccessor` redefines with a getter. */
const ACCESSOR_READS = {
  gaccmember: { text: "void window.devicePixelRatio" },
  gaccbare: { text: "void devicePixelRatio" },
  gacctypeof: { text: "void typeof window.devicePixelRatio" },
  gaccpattern: { prelude: ["let ratio = 0;"], text: "({ devicePixelRatio: ratio } = window)" },
};

/** ADR 0974: the read of `clock.now` (the getter of `fz-getter`) through a pattern: the runtime reports the call at the pattern's property, so the row stands there, not at `clock`. */
const GETTER_PATTERNS = {
  getterrename: { prelude: ["let n = 0;"], text: "({ now: n } = clock)" },
  getterdefault: { prelude: ["let now = 0;"], text: "({ now = 0 } = clock)" },
};

/** The site's text for a `pa-*` cell (misuse.mjs `describeUse` asks): the package's entry, or the action called where it is created. */
export function paUse(kind) {
  const base = { solid: [], web: [], namespaces: [], moduleLines: [], preludeLines: [], contextLine: undefined };
  if (kind === "inline") return { ...base, solid: ["action"], text: "action(function* () {})()" };
  // A global the library declares, replaced by a package imported for its effect: the call is of the global, no import names it.
  if (kind === "gproto") return { ...base, text: "new Float32Array(1).reverse()", importLines: [`import "${PA_PACKAGES[kind][0]}";`] };
  if (kind.startsWith("gwrite") || kind === "gview") return { ...base, text: 'btoa("x")', importLines: [`import "${PA_PACKAGES[kind][0]}";`] };
  // ADR 0974: the same replaced global reached by another route (a declaration the body holds first, then the call), or a redefined accessor read (the site is the read).
  const form = CALL_ROUTES[kind] ?? ACCESSOR_READS[kind] ?? PROTO_ROUTES[kind];
  if (form) return { ...base, text: form.text, preludeLines: form.prelude ?? [], importLines: [`import "${PA_PACKAGES[kind][0]}";`] };
  if (GETTER_PATTERNS[kind]) return { ...base, text: GETTER_PATTERNS[kind].text, preludeLines: GETTER_PATTERNS[kind].prelude, importLines: [`import { clock } from "${PA_PACKAGES[kind][0]}";`] };
  // The read of a getter: no call, the member evaluated for its value.
  if (kind === "getter") return { ...base, text: "void clock.now", importLines: [`import { clock } from "${PA_PACKAGES[kind][0]}";`] };
  // A package that publishes the global is imported for its effect before the entry's.
  const [publisher, entry] = PA_PACKAGES[kind].length === 2 ? PA_PACKAGES[kind] : [undefined, PA_PACKAGES[kind][0]];
  return { ...base, text: "paEnter()", importLines: [...(publisher ? [`import "${publisher}";`] : []), `import { paEnter } from "${entry}";`] };
}

/** Every `use` node of `nodes`, at any depth. */
function* uses(value) {
  if (Array.isArray(value)) for (const item of value) yield* uses(item);
  else if (value && typeof value === "object") {
    if (value.k === "use") yield value;
    for (const key of Object.keys(value)) if (key !== "k") yield* uses(value[key]);
  }
}

/**
 * The placement of `stmts` for a `pa-*` context (`{ body, view, packages }`), or undefined for any other. The site is the call itself: its `use` node is marked
 * (`pa`), and `describeUse` writes the package's entry or the inline action in its place, so the runtime reports the code at the site's own line.
 */
export function paContext(context, stmts, kit) {
  const parts = paParts(context);
  if (!parts) return undefined;
  for (const node of uses(stmts)) node.pa = parts.kind;
  const hosted = (() => {
    switch (parts.host) {
      case "body": return { body: stmts, view: [] };
      case "memo": { const m = kit.name("m"); return { body: [decl(m, prim("memo", fn([...stmts, ret(num(1))])))], view: [child(kit.name("v"), call(ref(m)))] }; }
      case "root": return { body: [expr(prim("root", fn(stmts)))], view: [] };
      case "timer": return { body: [expr(prim("timer", fn(stmts)))], view: [] };
      case "micro": return { body: [expr(prim("micro", fn(stmts)))], view: [] };
      default: throw new Error(`pa: no host ${parts.host}`);
    }
  })();
  const names = PA_PACKAGES[parts.kind];
  return { ...hosted, ...(names ? { packages: Object.fromEntries(names.map((name) => [name, paPackageFiles(parts.kind, name)])) } : {}) };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function paPlacement() {
  return {
    [PA_CODE]: {
      misuse: PA_CONTEXT_NAMES.filter((name) => PA_CONTEXTS[name] === "misuse"),
      legal: PA_CONTEXT_NAMES.filter((name) => PA_CONTEXTS[name] === "legal"),
    },
  };
}
