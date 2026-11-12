// The package hand-off shapes (batch 11, the B3 PR after #213): a flush in a function a package the build does not read as source may run as an action's step, an
// effect's apply or a leaf owner's callback. The census reasons of the flush accounting (no stepped action, no deferring primitive, no leaf scope) read the primitives
// the build's own table names, and a package whose source the build does not read can run what the project hands it under any of them. A route says how the function reaches
// the package:
//
//   ph-<route>-<host>          misuse   the package runs the function as the host: the flush raises the host's code
//   ph-<route>-<host>-never    legal    the same text, the package's function never runs what it is handed
//     routes  literal    `runStep(() => { <site> })`
//             ref        `function cb1() { <site> }`, `runStep(cb1)`: by reference
//             alias      `const cb1 = () => { <site> }; const again1 = cb1;`, `runStep(again1)`: through a copy
//             member     `runStepWith({ run: () => { <site> } })`: a member of an object literal
//             method     `runStepWith({ run() { <site> } })`: a method of one
//             holder     `const opts1 = { run: () => { <site> } };`, `runStepWith(opts1)`: a member of an object a const holds
//             array      `runStepAll([() => { <site> }])`: an element of an array literal
//             jsx        `<StepView render={() => { <site> }} />`: a prop of a component of the package
//             children   `<StepView>{() => { <site> }}</StepView>`: a child expression of one
//             wrap       `function wrap1(run) { runStep(run) }`, `wrap1(() => { <site> })`: through a project function's parameter
//             caller     `function helper1() { <site> }`, `runStep(() => helper1())`: a function the handed callback calls
//             return     `function make1() { return () => { <site> }; }`, `runStep(make1())`: a function a project function returns
//             call       `runStep.call(null, () => { <site> })`: a reflective call that forwards its arguments to the package's function
//           and, the flip of the default (the review of #216: a value counts as handed unless a fact of the program proves where it goes):
//             set        `const set1 = new Set(); set1.add(() => { <site> });`, `set1.forEach((f) => runStep(f))`: kept in a collection, read elsewhere
//             forof      `for (const f of [() => { <site> }]) runStep(f)`: an iterable's element
//             recv       `[() => { <site> }].forEach((f) => runStep(f))`: an array literal that is a method's receiver
//             resolve    `Promise.resolve(() => { <site> }).then((f) => runStep(f))`: a standard call that returns its argument
//             class      `class Job1 { run() { <site> } }`, `runStepWith(new Job1())`: a method of a class whose instance is handed
//             assign     `config.onStep = () => { <site> }`, `runStepConfig()`: an assignment into an object the package exports
//             props      `function Wrap1(props) { return <StepView render={props.render} /> }`, `<Wrap1 render={() => { <site> }} />`: a project component's props
//             getter     `const cb1 = () => { <site> }`, `runStepGet(() => cb1)`: a function the package runs returns it
//             reflect    `function wrap1(run) { runStep(run) }`, `wrap1.call(null, () => { <site> })`: a reflective call whose target is a project function
//             inline     `[1].forEach(cb1)` in a step the package runs: run inline by the standard library on the package's stack
//             param      `call1(cb1)` in a step the package runs, `function call1(g) { g() }`: run inline by a project function's parameter
//             untrack    `untrack(cb1)` in a step the package runs: run inline by a dialect primitive
//             dynamic    `<Dynamic component={StepView} render={() => { <site> }} />`: the component its `component` prop names
//   hosts   step (the package runs it in an action's step: FLUSH_IN_ACTION), apply (an effect's apply: FLUSH_IN_EFFECT_CALLBACK), leaf (a leaf owner's callback:
//           FLUSH_IN_TRACKED_QUEUE_CALLBACK)
//
// The package `fz-step` is written here, a manifest with no dependency on a Solid package (so that the checker does not read its source, which is the premise of the shape;
// `peerDependencies` naming solid-js would make it a dependency read as source: dependency_sources.rs), and exports one function per host and route kind, and a `skip*`
// that ignores what it is handed, the twin's. Chrome (rc.21, the real oracle) judges each cell: what it reports is what the checker is compared with. Recall contexts, the
// direct binding bare (`phShape`), not crossed by the census, out of api-fire's plan.
import { contain } from "./model.mjs";

export const PH_PACKAGE = "fz-step";
export const PH_ROUTES = ["literal", "ref", "alias", "member", "method", "holder", "array", "jsx", "children", "wrap", "caller", "return", "call", "set", "forof", "recv", "resolve", "class", "assign", "props", "getter", "reflect", "inline", "param", "untrack", "dynamic"];
export const PH_HOSTS = ["step", "apply", "leaf"];
/** The hosts each code is judged in: the code the host's callback reports for a flush in it. */
export const PH_CODES = {
  FLUSH_IN_EFFECT_CALLBACK: ["apply"],
  FLUSH_IN_ACTION: ["step"],
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: ["leaf"],
};
/** `{ [context]: "misuse" | "legal" }`. */
export const PH_CONTEXTS = Object.fromEntries(PH_ROUTES.flatMap((route) => PH_HOSTS.flatMap((host) => [[`ph-${route}-${host}`, "misuse"], [`ph-${route}-${host}-never`, "legal"]])));
export const PH_CONTEXT_NAMES = Object.keys(PH_CONTEXTS);

const NAME = new RegExp(`^ph-(${PH_ROUTES.join("|")})-(${PH_HOSTS.join("|")})(-never)?$`);
/** `{ route, host, never }` of a `ph-*` context name, or null. */
export const phParts = (context) => {
  const match = NAME.exec(context);
  return match ? { route: match[1], host: match[2], never: Boolean(match[3]) } : null;
};

/** The base shapes a `ph-*` cell takes: the direct binding, no layer. */
export function phShape(context, shape) {
  return !phParts(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

const UPPER = { step: "Step", apply: "Apply", leaf: "Leaf" };
/** The package function a route calls for a host: its name, the import it needs. */
const callee = (route, host, never) => {
  if (["jsx", "children", "props", "dynamic"].includes(route)) return never ? "SkipView" : `${UPPER[host]}View`;
  if (route === "getter") return never ? "skipRunGet" : `run${UPPER[host]}Get`;
  if (route === "assign") return never ? "skipRunConfig" : `run${UPPER[host]}Config`;
  const kind = ["member", "method", "holder", "class"].includes(route) ? "With" : route === "array" ? "All" : "";
  return never ? `skipRun${kind}` : `run${UPPER[host]}${kind}`;
};

/** The frame text of a route: `head` precedes the body (the callback's or the function's block), `close` follows it. */
const ROUTE_FRAMES = {
  literal: (f) => ({ head: `setTimeout(() => {\n  ${f}(() => `, close: `);\n});` }),
  ref: (f, n) => {
    const cb = n("cb");
    return { head: `function ${cb}() `, close: `\nsetTimeout(() => {\n  ${f}(${cb});\n});` };
  },
  alias: (f, n) => {
    const cb = n("cb");
    const again = n("again");
    return { head: `const ${cb} = () => `, close: `;\nconst ${again} = ${cb};\nsetTimeout(() => {\n  ${f}(${again});\n});` };
  },
  member: (f) => ({ head: `setTimeout(() => {\n  ${f}({ run: () => `, close: ` });\n});` }),
  method: (f) => ({ head: `setTimeout(() => {\n  ${f}({ run() `, close: ` });\n});` }),
  holder: (f, n) => {
    const opts = n("opts");
    return { head: `const ${opts} = { run: () => `, close: ` };\nsetTimeout(() => {\n  ${f}(${opts});\n});` };
  },
  array: (f) => ({ head: `setTimeout(() => {\n  ${f}([() => `, close: `]);\n});` }),
  jsx: (f) => ({ head: `setTimeout(() => {\n  document.body.append((<${f} render={() => `, close: `} />) as unknown as Node);\n});` }),
  children: (f) => ({ head: `setTimeout(() => {\n  document.body.append((<${f}>{() => `, close: `}</${f}>) as unknown as Node);\n});` }),
  wrap: (f, n) => {
    const wrap = n("wrap");
    return { head: `function ${wrap}(run: () => void) {\n  ${f}(run);\n}\nsetTimeout(() => {\n  ${wrap}(() => `, close: `);\n});` };
  },
  caller: (f, n) => {
    const helper = n("helper");
    return { head: `function ${helper}() `, close: `\nsetTimeout(() => {\n  ${f}(() => ${helper}());\n});` };
  },
  return: (f, n) => {
    const make = n("make");
    return { head: `function ${make}() {\n  return () => `, close: `;\n}\nsetTimeout(() => {\n  ${f}(${make}());\n});` };
  },
  call: (f) => ({ head: `setTimeout(() => {\n  ${f}.call(null, () => `, close: `);\n});` }),
  set: (f, n) => {
    const kept = n("kept");
    const each = n("each");
    return { head: `const ${kept} = new Set<() => void>();\n${kept}.add(() => `, close: `);\nsetTimeout(() => {\n  ${kept}.forEach((${each}) => {\n    ${f}(${each});\n  });\n});` };
  },
  forof: (f, n) => {
    const each = n("each");
    return { head: `setTimeout(() => {\n  for (const ${each} of [() => `, close: `]) {\n    ${f}(${each});\n  }\n});` };
  },
  recv: (f, n) => {
    const each = n("each");
    return { head: `setTimeout(() => {\n  [() => `, close: `].forEach((${each}) => {\n    ${f}(${each});\n  });\n});` };
  },
  resolve: (f, n) => {
    const each = n("each");
    return { head: `setTimeout(() => {\n  void Promise.resolve(() => `, close: `).then((${each}) => {\n    ${f}(${each});\n  });\n});` };
  },
  class: (f, n) => {
    const job = n("Job");
    return { head: `class ${job} {\n  run() `, close: `\n}\nsetTimeout(() => {\n  ${f}(new ${job}());\n});` };
  },
  assign: (f) => ({ head: `setTimeout(() => {\n  config.onStep = () => `, close: `;\n  ${f}();\n});` }),
  props: (f, n) => {
    const wrap = n("Wrap");
    return { head: `function ${wrap}(props: { render: () => void }) {\n  return <${f} render={props.render} />;\n}\nsetTimeout(() => {\n  document.body.append((<${wrap} render={() => `, close: `} />) as unknown as Node);\n});` };
  },
  getter: (f, n) => {
    const cb = n("cb");
    return { head: `const ${cb} = () => `, close: `;\nsetTimeout(() => {\n  ${f}(() => ${cb});\n});` };
  },
  reflect: (f, n) => {
    const wrap = n("wrap");
    return { head: `function ${wrap}(run: () => void) {\n  ${f}(run);\n}\nsetTimeout(() => {\n  ${wrap}.call(null, () => `, close: `);\n});` };
  },
  inline: (f, n) => {
    const cb = n("cb");
    const step = n("step");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${step}() {\n  [1].forEach(${cb});\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  param: (f, n) => {
    const cb = n("cb");
    const call = n("call");
    const step = n("step");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${call}(g: () => void) {\n  g();\n}\nfunction ${step}() {\n  ${call}(${cb});\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  untrack: (f, n) => {
    const cb = n("cb");
    const step = n("step");
    return { head: `const ${cb} = () => `, close: `;\nfunction ${step}() {\n  untrack(${cb});\n}\nsetTimeout(() => {\n  ${f}(${step});\n});` };
  },
  dynamic: (f) => ({ head: `setTimeout(() => {\n  document.body.append((<Dynamic component={${f}} render={() => `, close: `} />) as unknown as Node);\n});` }),
};

/** The other names a route imports from the package, and the import lines of the program's own libraries it needs. */
const ROUTE_NAMES = { assign: ["config"] };
const ROUTE_LINES = { untrack: ['import { untrack } from "solid-js";'], dynamic: ['import { Dynamic } from "@solidjs/web";'] };

/** The files of `fz-step`: a manifest naming no Solid package, so the checker does not read its source. */
export function phPackageFiles() {
  const text = (lines) => `${lines.join("\n")}\n`;
  const hosts = {
    step: (run) => `void action(function* () {\n    ${run}\n  })();`,
    apply: (run) => `createRoot(() => {\n    createEffect(() => 1, () => {\n      ${run}\n    });\n  });`,
    leaf: (run) => `createRoot(() => {\n    createTrackedEffect(() => {\n      ${run}\n    });\n  });`,
  };
  const js = ['import { action, createEffect, createRoot, createTrackedEffect } from "solid-js";', ""];
  const dts = [];
  for (const host of PH_HOSTS) {
    const up = UPPER[host];
    js.push(`export const run${up} = (cb) => {\n  ${hosts[host]("cb();")}\n};`);
    js.push(`export const run${up}With = (o) => {\n  ${hosts[host]("o.run();")}\n};`);
    js.push(`export const run${up}All = (fns) => {\n  ${hosts[host]("for (const f of fns) f();")}\n};`);
    // A component body runs under an owner, where an action's call is its own diagnostic (ACTION_CALLED_IN_OWNED_SCOPE, the package's): the view runs its host from a microtask.
    js.push(`export const run${up}Get = (get) => {\n  ${hosts[host]("get()();")}\n};`);
    js.push(`export const run${up}Config = () => {\n  ${hosts[host]("config.onStep?.();")}\n};`);
    js.push(`export const ${up}View = (props) => {\n  queueMicrotask(() => {\n    ${hosts[host]("props.render?.();\n    if (typeof props.children === \"function\") props.children();")}\n  });\n  return null;\n};`);
    dts.push(`export declare const run${up}: (cb: () => void) => void;`);
    dts.push(`export declare const run${up}With: (o: { run: () => void }) => void;`);
    dts.push(`export declare const run${up}All: (fns: Array<() => void>) => void;`);
    dts.push(`export declare const run${up}Get: (get: () => () => void) => void;`);
    dts.push(`export declare const run${up}Config: () => void;`);
    dts.push(`export declare const ${up}View: (props: { render?: () => void; children?: () => void }) => any;`);
  }
  js.push("export const config = {};", "export const skipRun = (cb) => { void cb; };", "export const skipRunWith = (o) => { void o; };", "export const skipRunAll = (fns) => { void fns; };", "export const skipRunGet = (get) => { void get; };", "export const skipRunConfig = () => { void config; };", "export const SkipView = (props) => { void props; return null; };");
  dts.push("export declare const config: { onStep?: () => void };", "export declare const skipRun: (cb: () => void) => void;", "export declare const skipRunWith: (o: { run: () => void }) => void;", "export declare const skipRunAll: (fns: Array<() => void>) => void;", "export declare const skipRunGet: (get: () => () => void) => void;", "export declare const skipRunConfig: () => void;", "export declare const SkipView: (props: { render?: () => void; children?: () => void }) => any;");
  return {
    // No `dependencies` or `peerDependencies` on a Solid package: the build does not read this package's source (the program's own `solid-js` serves its import at run time).
    "package.json": `${JSON.stringify({ name: PH_PACKAGE, version: "1.0.0", type: "module", exports: { ".": { types: "./index.d.ts", default: "./index.js" } } }, null, 2)}\n`,
    "index.js": text(js),
    "index.d.ts": text(dts),
  };
}

/** The placement of `stmts` for a `ph-*` context (`{ body, view, packages, importLines }`), or undefined for any other. */
export function phContext(context, stmts, kit) {
  const parts = phParts(context);
  if (!parts) return undefined;
  const name = callee(parts.route, parts.host, parts.never);
  const frame = ROUTE_FRAMES[parts.route](name, kit.name);
  return {
    body: [contain("frame", 9, stmts, frame)],
    view: [],
    packages: { [PH_PACKAGE]: phPackageFiles() },
    importLines: [`import { ${[name, ...(ROUTE_NAMES[parts.route] ?? [])].join(", ")} } from "${PH_PACKAGE}";`, ...(ROUTE_LINES[parts.route] ?? [])],
  };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function phPlacement() {
  return Object.fromEntries(Object.entries(PH_CODES).map(([code, hosts]) => {
    const contexts = PH_CONTEXT_NAMES.filter((name) => hosts.includes(phParts(name).host));
    return [code, { misuse: contexts.filter((name) => PH_CONTEXTS[name] === "misuse"), legal: contexts.filter((name) => PH_CONTEXTS[name] === "legal") }];
  }));
}
