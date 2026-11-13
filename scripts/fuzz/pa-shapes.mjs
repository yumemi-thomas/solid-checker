// ADR 0972: a call into code of a package the build does not read as source, written where an owner may be active. The package may call Solid's `action` itself,
// so the call throws ACTION_CALLED_IN_OWNED_SCOPE although the program names no action there. A cell crosses what the package is with where the call is written:
//
//   pa-<package>-<host>
//     packages  reach    `fz-act`: its `paEnter()` calls `action(function* () {})()`; its manifest names no Solid package, so the build does not read it
//               pure     `fz-calm`: names no Solid package in its closure, loads nothing by a computed name, reads only the libraries' globals (the checker clears it)
//               dyn      `fz-load`: loads `import(name)` on a branch that never runs (the checker cannot clear it; Chrome is quiet)
//               inline   no package: the program's own `action(function* () {})()`, an action called where it is created (the second hole of ADR 0972)
//               winself  `fz-pub` (a side-effect import that reaches Solid and writes `window.__paAct`) and `fz-winself`, whose `paEnter()` calls `window.self.__paAct()`:
//                        the global object through its own member (ADR 0973: the scan followed one member and cleared it)
//               docview  `fz-pub` and `fz-docview`, whose `paEnter()` calls `document.defaultView.__paAct()` (ADR 0973)
//               getter   `fz-getter`: `clock` has `get now()` that calls the action; the site is the read `void clock.now`, no call (ADR 0973)
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

export const PA_PACKAGE_KINDS = ["reach", "pure", "dyn", "inline", "winself", "docview", "getter"];
export const PA_HOSTS = ["body", "memo", "root", "timer", "micro"];
const OWNED_HOSTS = ["body", "memo", "root"];
const THROWING_KINDS = ["reach", "inline", "winself", "docview", "getter"];
export const PA_CODE = "ACTION_CALLED_IN_OWNED_SCOPE";
/** The packages each kind imports from, in the order its site writes them (`inline` imports none); the last holds the entry. */
export const PA_PACKAGES = { reach: ["fz-act"], pure: ["fz-calm"], dyn: ["fz-load"], winself: ["fz-pub", "fz-winself"], docview: ["fz-pub", "fz-docview"], getter: ["fz-getter"] };
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
  }[name];
  return {
    "package.json": `${JSON.stringify({ name, version: "1.0.0", type: "module", exports: { ".": { types: "./index.d.ts", default: "./index.js" } } }, null, 2)}\n`,
    "index.js": `${files.js.join("\n")}\n`,
    "index.d.ts": `${files.dts.join("\n")}\n`,
  };
}

/** The site's text for a `pa-*` cell (misuse.mjs `describeUse` asks): the package's entry, or the action called where it is created. */
export function paUse(kind) {
  const base = { solid: [], web: [], namespaces: [], moduleLines: [], preludeLines: [], contextLine: undefined };
  if (kind === "inline") return { ...base, solid: ["action"], text: "action(function* () {})()" };
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
