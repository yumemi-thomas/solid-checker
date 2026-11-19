// The context-provider children axis (ADR 1123, the Fable review of round 1, F2 and F3): the `context-provider-children` arm reads a dialect provider's written children
// in place, where the provider is created. rc.21's provider returns `children(() => props.children)`, a memo its READER runs, and its tag is only a provider when the
// binding is the context object. Each cell holds the code's own site in an immediately invoked arrow written as a provider-like element's child expression, and the
// element's children are read in a tracked effect, where rc.21 reports the flush (FLUSH_IN_TRACKED_QUEUE_CALLBACK). The build before the fix certified each one.
//
//   cp-stored   the provider element of a local `const` context stored in a `const` of a component, then called in `createTrackedEffect` (F3)
//   cp-pattern  `const { defaultValue: CpP = CpR } = createContext(CpR)`: the tag is a pattern's name, here the reader component CpR, no provider (F2)
//   cp-let      `let CpW = createContext(1)` and later `CpW = CpR`: a written `let` that holds the reader component when the tag runs (F2)
//
// CpR, the reader, reads `props.children` in `createTrackedEffect`. Each cell's element sits in a module-level component (`CpH`) the host's view renders by its tag.
// (A component child would be created in the tracked effect, PRIMITIVE_IN_FORBIDDEN_SCOPE, before its flush runs.)
// Recall contexts, the direct binding bare (`cpShape`), one program per cell, not crossed by the census, out of api-fire's plan.
import { contain, domExpr, expr, num, propEl } from "./model.mjs";

const READER = (r) => `function ${r}(props: { value: number; children?: unknown }) { createTrackedEffect(() => { void props.children; }); return <p>r</p>; }`;

/** Each cell: the module declarations (given the names), and the host component's body around the child (an immediately invoked arrow written as the
 * element's child expression, which holds the site): `[before, after]`. */
const CELLS = {
  "cp-stored": {
    module: ({ c }) => [`const ${c} = createContext(1)`],
    host: ({ c }) => [`const pe = <${c} value={1}>{(() => `, `)()}</${c}>; createTrackedEffect(() => { const run: unknown = pe; if (typeof run === "function") run(); }); return <p>h</p>;`],
  },
  "cp-pattern": {
    module: ({ r, c }) => [READER(r), `const { defaultValue: ${c} = ${r} } = createContext(${r})`],
    host: ({ c }) => [`return <${c} value={1}>{(() => `, `)()}</${c}>;`],
  },
  "cp-let": {
    module: ({ r, c }) => [READER(r), `let ${c} = createContext(1)`, `${c} = ${r} as unknown as typeof ${c}`],
    host: ({ c }) => [`return <${c} value={1}>{(() => `, `)()}</${c}>;`],
  },
};

export const CP_CONTEXTS = Object.fromEntries(Object.keys(CELLS).map((name) => [name, "misuse"]));
export const CP_CONTEXT_NAMES = Object.keys(CP_CONTEXTS);
export const CP_CODES = ["FLUSH_IN_TRACKED_QUEUE_CALLBACK"];

/** The base shapes a `cp-*` cell takes: the direct binding, no layer. */
export function cpShape(context, shape) {
  return !CP_CONTEXT_NAMES.includes(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/** The placement of `stmts` (the code's own site) for a `cp-*` context (`{ hoisted, body, view }`), or undefined for any other. */
export function cpContext(context, stmts, kit) {
  if (!CP_CONTEXT_NAMES.includes(context)) return undefined;
  const cell = CELLS[context];
  const names = { r: kit.name("CpR"), c: kit.name("CpC"), h: kit.name("CpH") };
  const solid = ["createContext", "createTrackedEffect"];
  const [before, after] = cell.host(names);
  const host = contain("frame", 9, stmts, { head: `function ${names.h}(_props: { a?: unknown }) { ${before}`, close: `${after} }`, after: ["return <p>c</p>;"], imports: solid });
  // The arrow returns an element: a provider's `children` is typed `JSX.Element` (rc.21), which `void` is not.
  return {
    hoisted: [...cell.module(names).map((text) => expr(domExpr(text, undefined, solid))), host],
    body: [],
    view: [propEl(names.h, num(1))],
  };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function cpPlacement() {
  return Object.fromEntries(CP_CODES.map((code) => [code, { misuse: CP_CONTEXT_NAMES, legal: [] }]));
}
