// ADR 0972: a call into code of a package the build does not read as source, written where an owner may be active. The package may call Solid's `action` itself,
// so the call throws ACTION_CALLED_IN_OWNED_SCOPE although the program names no action there. A cell crosses what the package is with where the call is written:
//
//   pa-<package>-<host>
//     packages  reach    `fz-act`: its `paEnter()` calls `action(function* () {})()`; its manifest names no Solid package, so the build does not read it
//               pure     `fz-calm`: names no Solid package in its closure, loads nothing by a computed name, reads only the libraries' globals (the checker clears it)
//               dyn      `fz-load`: loads `import(name)` on a branch that never runs (the checker cannot clear it; Chrome is quiet)
//               inline   no package: the program's own `action(function* () {})()`, an action called where it is created (the second hole of ADR 0972)
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

export const PA_PACKAGE_KINDS = ["reach", "pure", "dyn", "inline"];
export const PA_HOSTS = ["body", "memo", "root", "timer", "micro"];
const OWNED_HOSTS = ["body", "memo", "root"];
const THROWING_KINDS = ["reach", "inline"];
export const PA_CODE = "ACTION_CALLED_IN_OWNED_SCOPE";
/** The package each kind imports from (`inline` imports none). */
export const PA_PACKAGES = { reach: "fz-act", pure: "fz-calm", dyn: "fz-load" };
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

/** The files of each package: a manifest that names no Solid package (the build does not read its source; the program's own `solid-js` serves the import at run time). */
export function paPackageFiles(kind) {
  const js = {
    reach: ['import { action } from "solid-js";', "", "export const paEnter = () => {", "  void action(function* () {})();", "  return 1;", "};"],
    pure: ["const twice = (n) => n * 2;", "", "export const paEnter = () => twice([1].length) + Math.max(1, 2);"],
    dyn: ["export const paEnter = () => {", '  const name = ["./", "never.js"].join("");', '  if (name === "") void import(name);', "  return 1;", "};"],
  }[kind];
  return {
    "package.json": `${JSON.stringify({ name: PA_PACKAGES[kind], version: "1.0.0", type: "module", exports: { ".": { types: "./index.d.ts", default: "./index.js" } } }, null, 2)}\n`,
    "index.js": `${js.join("\n")}\n`,
    "index.d.ts": "export declare const paEnter: () => number;\n",
  };
}

/** The site's text for a `pa-*` cell (misuse.mjs `describeUse` asks): the package's entry, or the action called where it is created. */
export function paUse(kind) {
  const base = { solid: [], web: [], namespaces: [], moduleLines: [], preludeLines: [], contextLine: undefined };
  return kind === "inline"
    ? { ...base, solid: ["action"], text: "action(function* () {})()" }
    : { ...base, text: "paEnter()", importLines: [`import { paEnter } from "${PA_PACKAGES[kind]}";`] };
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
  const pkg = PA_PACKAGES[parts.kind];
  return { ...hosted, ...(pkg ? { packages: { [pkg]: paPackageFiles(parts.kind) } } : {}) };
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
