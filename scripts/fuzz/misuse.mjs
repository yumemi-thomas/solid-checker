// The misuse profile: one program per Solid dev diagnostic code that a rule claims, built to provoke exactly that code at one marked
// site (a `use` node), in a chosen context, through a chosen binding form.
//
//   code      a key of SPEC (the claimed codes of the policy table, codes.mjs)
//   context   where the call sits: a component body, a memo or effect compute, an effect or render-effect apply, createTrackedEffect,
//             onSettled, untrack, createRoot, runWithOwner(null), a handler, a timer, a promise callback, a microtask, after an await,
//             a JSX child (plain, under Show, in a For row), an action body, an async memo or effect compute
//   layer     none, a closure (named, called), a same-file helper taking the callback, an IIFE, two of them, or the helper in
//             another module (`helpers.ts`, imported by name: the checker must follow the call across files)
//   binding   the call reaches its API through a direct import, a namespace import, an alias, a destructuring, or a wrapper function
//
// A cell of the recall matrix (matrix.mjs) is (code, context). `SPEC[code].contexts` lists the contexts the template is meant to fire
// in; a context where the call is legal (a write in a handler) is not in the list. `typedAway` codes have no template: the published
// typings reject every program that could provoke them, so `tsc` already reports it (invariant 1) and the checker stays silent.
import {
  walk, signal, storeSignal, memoSignal, actionSignal, asyncSignal, storeOptSignal, storeNestedSignal, num, ref, call, iife, fn, prim, expr, decl, ret, awaitStmt, read, write,
  button, child, show, forEach, use, contain,
} from "./model.mjs";
import { codePolicy, policyOf } from "./codes.mjs";

/** The binding forms. `direct` is the baseline; the others change how the call reaches the API, never what it does. */
export const BINDINGS = ["direct", "namespace", "alias", "destructured", "wrapper"];
export const LAYERS = ["none", "closure", "helper", "iife", "closure+helper", "module-helper"];
/**
 * Where the misuse component is mounted: by the entry's own `render` (`root`, every program before the mount axis), or as a child
 * an exported parent renders: plainly (`child`), under `<Show when={true}>` (`show`), or in a one-row `<For>` (`for`). Grid-only
 * (`--mounts`), never drawn: a root program keeps its seed, id and text.
 */
export const MOUNTS = ["root", "child", "show", "for"];

/**
 * Where the values the misuse uses come from (the template's `setup` signals; a context's own trigger signal stays local). `local`
 * (every program before the source axes): the misuse component creates them. The others, grid-only (`--sources`), never drawn, so a
 * local program keeps its seed, id and text:
 *   props    an exported parent `P1` creates them and renders `<C1 w1={w1} setW1={setW1} />`; the component reads `props.w1`
 *            (`prop`), destructures its parameter (`prop-destructured`), reads them through `merge({ extra: 1 }, props)` or
 *            `omit(props, "extra")` (rc.17 has no `mergeProps`/`splitProps`), or gets them through `M1`, which spreads its props
 *            on `<C1 {...props} />` (`prop-spread`). `prop-value` passes a read's value (`w1={w1()}`, a getter the compiled JSX
 *            makes) and the misuse reads `props.w1`.
 *   context  the parent provides them (`<ValCtx value={{ w1, setW1 }}>`) and the component reads `useContext(ValCtx)`, as an object
 *            (`ctx.setW1`) or destructured.
 *   module   `helpers.ts` creates them at module scope (`export const [w1, setW1] = createSignal(0)`), or a hook there does
 *            (`useVals()`), destructured or as an object (`vals.setW1`); or App.tsx holds one module-scope result of the hook
 *            (`shared.w1`) beside the body's own call of it (`module-hook-shared`, the ADR 0511 review's false positive: a read
 *            through another instance), its async memos settled before the mount.
 *   async    the async memo of an async template, created in the parent and read in the child: passed as an accessor, as its
 *            value (`a1={a1()}`), by context, or as an accessor under the parent's `<Loading>`.
 * `props` and `context` take the templates whose values are not async (async values are the `async` axis's); `module` takes all.
 */
export const SOURCE_AXES = {
  props: ["prop", "prop-destructured", "prop-merge", "prop-omit", "prop-spread", "prop-value"],
  context: ["context", "context-destructured"],
  module: ["module", "module-hook", "module-hook-member", "module-hook-shared"],
  async: ["async-prop", "async-value", "async-context", "async-loading"],
};
export const SOURCES = ["local", ...Object.values(SOURCE_AXES).flat()];
/** Sources the grids enumerate and a random draw never takes (as `gridVariants`): random programs and combinations keep theirs. */
export const GRID_ONLY_SOURCES = ["module-hook-shared"];
/** A source whose values a parent component creates (the component is then rendered by `P1`, not exported). */
export const parentSource = (source) => source !== "local" && !source.startsWith("module");
/** A source that hands the component a read's value, not the accessor: only a read written as the misuse itself can use it. */
const valueSource = (source) => source === "prop-value" || source === "async-value";

/**
 * The sources a (code, variant, binding) can take: the template must have values (`setup`); `async` sources need an async value and
 * `props`/`context` need none; a value source needs a bare read (`w1()`) at the site and a binding that keeps the read there (an alias
 * of `props.w1` would read it on the alias's line); a template may refuse a source in a context (`sourcesNotIn`).
 */
export function sourcesOf(code, variant, binding, context) {
  const spec = SPEC[code];
  const excluded = spec.sourcesNotIn?.[context] ?? [];
  const sigs = spec.setup(variant, (prefix) => `${prefix}1`);
  if (!sigs.length) return ["local"];
  const isAsync = sigs.some((sig) => sig.kind === "async");
  const target = spec.call({ code, variant }, sigs);
  const bareRead = target.kind === "local" && !target.args && sigs.some((sig) => sig.name === target.name && [undefined, "memo", "async"].includes(sig.kind));
  // A variant one source carries (`variantSources`): the settled async memo is read through the module-scope hook result only.
  const only = spec.variantSources?.[variant];
  return SOURCES.filter((source) => {
    if (only) return only.includes(source);
    // Its async memos settle before the mount, which takes an async template's code away (the `settled` variant is that case).
    if (source === "module-hook-shared" && isAsync) return false;
    if (source === "local") return true;
    if (excluded.includes(source)) return false;
    if (source.startsWith("async-") ? !isAsync : (!source.startsWith("module") && isAsync)) return false;
    if (valueSource(source)) return bareRead && ["direct", "wrapper"].includes(binding);
    return true;
  });
}

/** `--sources a,b`: source names or axis names (`props`, `context`, `module`, `async`); default `local` only. */
export function sourcesOption(value) {
  const sources = (value ?? "local").split(",").flatMap((name) => SOURCE_AXES[name] ?? [name]);
  for (const source of sources) if (!SOURCES.includes(source)) throw new Error(`--sources: unknown source ${source} (${[...Object.keys(SOURCE_AXES), ...SOURCES].join(", ")})`);
  return [...new Set(sources)];
}

/**
 * How the component names a value of a source: the text of the value `name` the component created it as. `props.w1` (an object
 * the component reads), `w1` (a destructured or imported binding), and `read: "value"` for a value source (the read is `props.w1`,
 * not a call).
 */
export function sourcedName(source, name) {
  switch (source) {
    case "prop": case "prop-spread": case "prop-value": case "async-prop": case "async-value": case "async-loading": return `props.${name}`;
    case "prop-merge": return `merged.${name}`;
    case "prop-omit": return `rest.${name}`;
    case "context": case "async-context": return `ctx.${name}`;
    case "module-hook-member": return `vals.${name}`;
    case "module-hook-shared": return `shared.${name}`;
    default: return name;
  }
}
export const sourcedRead = (source) => (valueSource(source) ? "value" : "call");

/** Contexts where nothing owns the code that runs: a handler, a timer, a promise or microtask callback, after an await, runWithOwner(null). */
const UNOWNED = ["handler", "timer", "then", "micro", "async", "owner-null"];
/** Contexts that run under a children-capable owner while the code runs, so a write or an action call is refused. */
const OWNED_PURE = ["body", "memo", "effect-compute", "render-compute", "render-apply", "root", "untrack", "jsx", "show", "for"];
/** The leaf scopes that forbid child owners. */
const LEAF = ["settled", "tracked"];

export const MODULE = { "solid-js": "S", "@solidjs/web": "W" };

/** An API call: `name` from `module`; `args` is the argument text; `locals` when the arguments name component-scope variables. */
const api = (name, args = "", { module = "solid-js", locals = false } = {}) => ({ kind: "api", module, name, args, locals });
/** A call of a component-scope function (a setter, an accessor, an action). */
const local = (name, args = "") => ({ kind: "local", name, args });
/** A statement on a component-scope object (a store write): `text(object)`. */
const member = (object, text) => ({ kind: "member", object, text });

/**
 * Per code: the variants, the contexts and bindings to draw from, the component-level signals the call needs (`setup`), the call
 * itself (`call`), and the spec's note on what it takes to fire.
 *   setup(variant, name) -> signals   (name(prefix) -> a fresh identifier)
 *   call(node, sigs) -> api | local | member
 */
export const SPEC = {
  REACTIVE_WRITE_IN_OWNED_SCOPE: { legal: ["handler", "timer", "then", "micro", "effect-apply"],
    variants: ["signal", "store", "refresh"], contexts: OWNED_PURE, bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => [variant === "store" ? storeSignal(name("w"), 0) : variant === "refresh" ? memoSignal(name("m")) : signal(name("w"), 0, "probe")],
    call: (node, [sig]) => (node.variant === "store" ? local(sig.setter, "(s) => { s.v = 7; }") : node.variant === "refresh" ? api("refresh", sig.name, { locals: true }) : local(sig.setter, "7")),
    // refresh is an API; the other two are local functions, which have no namespace form.
    bindingsFor: (variant) => (variant === "refresh" ? BINDINGS : ["direct", "alias", "destructured", "wrapper"]),
  },
  STORE_WRITE_OUTSIDE_SETTER: { legal: ["handler", "timer", "then"],
    variants: ["assign", "delete"],
    contexts: [...OWNED_PURE.filter((c) => !["show", "for"].includes(c)), "effect-apply", "handler", "timer", "then", "async"],
    bindings: ["direct", "alias", "wrapper"],
    // Grid-only (`gridVariantsOf`): writes past the root (a nested record's field, an array mutator), and a write deferred out of the
    // store's own setter callback (a microtask, a timer, a promise callback), which runs once the draft has closed and is dropped
    // (ADR 0476; Chrome-witnessed in probes/store-setter). A deferred write calls the setter, which is itself a defect in an owned
    // scope, so it runs only where the call is legal (`variantContexts`).
    gridVariants: ["nested-assign", "nested-push", "deferred-micro", "deferred-timer", "deferred-then"],
    variantContexts: { "deferred-micro": ["handler", "timer", "then"], "deferred-timer": ["handler", "timer", "then"], "deferred-then": ["handler", "timer", "then"] },
    // Legal (legal.mjs): a write through the draft, and one through the read proxy inside its own setter, synchronously; both commit.
    legalVariants: ["draft", "in-setter"],
    setup: (variant, name) => [variant === "delete" ? storeOptSignal(name("s"), 0) : variant?.startsWith("nested") ? storeNestedSignal(name("s"), 0) : storeSignal(name("s"), 0)],
    call: (node, [sig]) => member(sig.name, (object) => {
      const deferred = { "deferred-micro": (w) => `queueMicrotask(() => { ${w}; })`, "deferred-timer": (w) => `setTimeout(() => { ${w}; }, 0)`, "deferred-then": (w) => `Promise.resolve().then(() => { ${w}; })` }[node.variant];
      if (deferred) return `${sig.setter}(() => { ${deferred(`${object}.v = 7`)}; })`;
      if (node.variant === "in-setter") return `${sig.setter}(() => { ${object}.v = 7; })`;
      if (node.variant === "draft") return `${sig.setter}((draft) => { draft.v = 7; })`;
      return node.variant === "delete" ? `delete ${object}.v` : node.variant === "nested-assign" ? `${object}.a.v = 7` : node.variant === "nested-push" ? `${object}.items.push(1)` : `${object}.v = 7`;
    }),
  },
  // `settled`: an async memo read after it settled (ADR 0511 review: probe pending-async/hook-shared-instance, rc.17 warns this code in an
  // apply, no pending throw), through the module-scope hook result beside the body's own call.
  STRICT_READ_UNTRACKED: { legal: ["memo", "effect-compute", "render-compute", "jsx", "untrack"],
    // `settled` is a grid variant (never drawn at random, so random programs and combinations' extras keep theirs) and a legal one.
    variants: ["signal", "memo"], gridVariants: ["settled"], legalVariants: ["signal", "memo", "settled"],
    contexts: ["body", "effect-apply", "root", "render-apply"], bindings: ["direct", "alias", "destructured", "wrapper"],
    variantContexts: { settled: ["effect-apply", "render-apply"] }, variantSources: { settled: ["module-hook-shared"] },
    setup: (variant, name) => [variant === "memo" ? memoSignal(name("m")) : variant === "settled" ? asyncSignal(name("a")) : signal(name("r"), 3, "src")],
    call: (node, [sig]) => local(sig.name),
  },
  NO_OWNER_CLEANUP: { legal: ["body", "memo", "effect-compute", "render-compute", "root"], contexts: UNOWNED, bindings: BINDINGS, setup: () => [], call: () => api("onCleanup", "() => {}") },
  NO_OWNER_EFFECT: { legal: ["body", "root"],
    variants: ["effect", "render"], contexts: UNOWNED, bindings: BINDINGS, setup: () => [],
    call: (node) => api(node.variant === "render" ? "createRenderEffect" : "createEffect", "() => 1, () => {}"),
  },
  NO_OWNER_BOUNDARY: {
    variants: ["loading", "errored"], contexts: UNOWNED, bindings: BINDINGS, setup: () => [],
    call: (node) => api(node.variant === "errored" ? "Errored" : "Loading", "{ fallback: 0, children: 1 }"),
  },
  NO_OWNER_CONTEXT: { legal: ["body", "memo", "root"], contexts: UNOWNED, bindings: BINDINGS, setup: () => [], contextLine: "const Ctx = createContext(7);", call: () => api("useContext", "Ctx") },
  CONTEXT_NOT_FOUND: {
    contexts: ["body", "memo", "render-compute", "root", "jsx", "show", "for"], bindings: BINDINGS, setup: () => [],
    contextLine: "const Ctx = createContext<number>();", call: () => api("useContext", "Ctx"),
  },
  SETTLED_CLEANUP_UNOWNED: { legal: ["body", "root"], contexts: [...UNOWNED, "effect-apply"], bindings: BINDINGS, setup: () => [], call: () => api("onSettled", "() => () => {}") },
  CLEANUP_IN_FORBIDDEN_SCOPE: { legal: ["body", "root"], contexts: LEAF, bindings: BINDINGS, setup: () => [], call: () => api("onCleanup", "() => {}") },
  PRIMITIVE_IN_FORBIDDEN_SCOPE: { legal: ["body", "root"],
    variants: ["memo", "root", "effect"], contexts: LEAF, bindings: BINDINGS, setup: () => [],
    call: (node) => (node.variant === "root" ? api("createRoot", "() => 1") : node.variant === "effect" ? api("createEffect", "() => 1, () => {}") : api("createMemo", "() => 1")),
  },
  ASYNC_OUTSIDE_LOADING_BOUNDARY: {
    contexts: ["jsx", "show", "for"], bindings: ["direct", "alias", "destructured", "wrapper"], placeless: true,
    setup: (variant, name) => [asyncSignal(name("a"))], call: (node, [sig]) => local(sig.name),
  },
  // rc.17 dev-shared.js: a pending read inside createTrackedEffect or onSettled serves the committed value (and throws while uninitialized).
  PENDING_ASYNC_FORBIDDEN_SCOPE: {
    contexts: LEAF, bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => [asyncSignal(name("a"))], call: (node, [sig]) => local(sig.name),
  },
  // The `-guarded` applies (ADR 0504 review): the read sits behind a guard that returns on the first run, which is the only run;
  // `-closure`: the guard is in a closure the apply calls. `-rerun`: the compute reads a signal a button sets to 2, so the apply runs again
  // and the guarded read executes (the source settled by then): Chrome then judges the strict read the checker reports there.
  // A module-scope async memo is created with no owner and is still pending when a `-guarded-rerun` apply runs again (Chrome reports
  // the read there): those two legal contexts do not take the `module` source.
  PENDING_ASYNC_UNTRACKED_READ: { sourcesNotIn: { "effect-apply-guarded-rerun": ["module"], "render-apply-guarded-rerun": ["module"] }, legal: ["memo", "effect-apply-guarded", "render-apply-guarded", "effect-apply-guarded-closure", "render-apply-guarded-closure", "effect-apply-guarded-rerun", "render-apply-guarded-rerun"],
    contexts: ["body", "root", "effect-apply", "render-apply"], bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => [asyncSignal(name("a"))], call: (node, [sig]) => local(sig.name),
  },
  SYNC_NODE_RECEIVED_ASYNC: {
    variants: ["memo", "effect"], contexts: ["body", "root", "untrack", "memo", "jsx"], bindings: BINDINGS, setup: () => [],
    call: (node) => (node.variant === "effect" ? api("createEffect", "async () => 1, () => {}, { sync: true }") : api("createMemo", "async () => 1, { sync: true }")),
  },
  UNTRACKED_READ_AFTER_AWAIT: {
    contexts: ["memo-async", "effect-async"], bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => [signal(name("r"), 3, "src")], call: (node, [sig]) => local(sig.name),
  },
  // Defect codes no rule claims (codes.mjs `defect-unclaimed`): measured for "never quieter than Solid"; a silent checker is a false clean.
  // rc.17 dev-shared.js `assertNotInEffectCallback`: a memo, effect or root created with no owner in an effect callback belongs to nobody.
  PRIMITIVE_IN_EFFECT_CALLBACK: { legal: ["body", "root"],
    // Effect apply only: in a render effect's apply rc.17 does not report it (the fast path fires it in every effect-apply cell).
    variants: ["memo", "root", "effect"], contexts: ["effect-apply"], bindings: BINDINGS, setup: () => [],
    call: (node) => (node.variant === "root" ? api("createRoot", "() => 1") : node.variant === "effect" ? api("createEffect", "() => 1, () => {}") : api("createMemo", "() => 1")),
  },
  ASYNC_STORE_SETTER: {
    typedAway: true, contexts: [], bindings: [], setup: () => [], call: () => api("createStore"),
    probe: 'import { createStore } from "solid-js";\nexport function C1() {\n  const [s1, setS1] = createStore({ v: 0 });\n  return <button onClick={() => { setS1(async (s) => { await Promise.resolve(); s.v = 7; }); }}>{s1.v}</button>;\n}\n',
    why: "the typings reject it: the setter callback is `(state) => void | T`, so an async callback is TS2345 and tsc already reports it",
  },
  MISSING_EFFECT_FN: {
    typedAway: true, contexts: [], bindings: [], setup: () => [], call: () => api("createEffect", "() => 1"),
    probe: 'import { createEffect } from "solid-js";\nexport function C1() {\n  createEffect(() => 1);\n  return <div></div>;\n}\n',
    why: "createEffect(compute, effectFn) takes two functions: the typings reject the call (TS2554 \"Expected 2-3 arguments\"), so tsc already reports it",
  },
  ACTION_CALLED_IN_OWNED_SCOPE: { legal: ["handler", "timer", "then", "micro"],
    contexts: OWNED_PURE, bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => { const target = signal(name("w"), 0, "probe"); return [target, actionSignal(name("a"), target.name, 7)]; },
    call: (node, [, act]) => local(act.name),
  },
  RESOLVE_IN_REACTIVE_SCOPE: { legal: ["handler", "timer"], contexts: ["memo", "render-compute"], bindings: BINDINGS, setup: () => [], call: () => api("resolve", "() => 1") },
  UNTIL_IN_REACTIVE_SCOPE: { legal: ["handler", "timer"], contexts: ["memo", "render-compute"], bindings: BINDINGS, setup: () => [], call: () => api("until", "() => 1") },
  FLUSH_IN_ACTION: { legal: ["handler", "timer"], contexts: ["action"], bindings: BINDINGS, setup: () => [], call: () => api("flush") },
  FLUSH_IN_EFFECT_CALLBACK: { legal: ["handler", "timer"], contexts: ["effect-apply"], bindings: BINDINGS, setup: () => [], call: () => api("flush") },
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: { legal: ["handler", "timer"], contexts: LEAF, bindings: BINDINGS, setup: () => [], call: () => api("flush") },
  DYNAMIC_STATIC_PROMISE: {
    contexts: ["body", "root", "untrack", "memo"], bindings: BINDINGS, setup: () => [],
    call: () => api("dynamic", '() => Promise.resolve("p" as const), { static: true }', { module: "@solidjs/web" }),
  },
};

/**
 * The codes the matrix measures, in the policy table's order: the defects, claimed by a rule or not ("never quieter than Solid": an
 * unclaimed defect the checker is silent on is a false clean too). An advisory code (Solid documents it as legal) is not measured.
 */
export const MISUSE_CODES = Object.keys(codePolicy).filter((code) => SPEC[code] && ["defect-claimed", "defect-unclaimed"].includes(policyOf(code)?.disposition));
/** The codes that have a template (not typed away). */
export const TEMPLATE_CODES = MISUSE_CODES.filter((code) => !SPEC[code].typedAway);
export const contextsOf = (code) => SPEC[code].contexts;
export const bindingsOf = (code, variant) => SPEC[code].bindingsFor?.(variant) ?? SPEC[code].bindings;
export const variantsOf = (code) => SPEC[code].variants ?? [null];
/** The variants of a code's legal programs (legal.mjs): `legalVariants`, else the misuse variants. */
export const legalVariantsOf = (code) => SPEC[code].legalVariants ?? variantsOf(code);
/** The codes that have legal contexts. */
export const LEGAL_CODES = () => TEMPLATE_CODES.filter((code) => SPEC[code].legal?.length);
/** The variants the grid enumerates: the random draw's, then `gridVariants` (never drawn at random, so the random matrix keeps its programs). */
export const gridVariantsOf = (code) => [...variantsOf(code), ...(SPEC[code].gridVariants ?? [])];

/**
 * Bindings of a component-scope value (a setter, an accessor, an action, a store) in the shapes real code hands one on: a hook that
 * returns it in an object (`const { useIt } = useThing()`), the hook's result used through a member (`thing.useIt()`), a tuple slot
 * (`const [, useIt] = pair`). Grid-only (`gridBindingsOf`): never drawn at random, so the random matrix's programs and baseline stay.
 * The `destructured` binding of a value is an object literal (`const { useIt } = { useIt: r1 }`), which real code rarely writes.
 */
export const VALUE_BINDINGS = ["hook", "hook-member", "tuple"];
const SAMPLE_SIGS = [{ name: "x", setter: "setX" }, { name: "a", setter: "setA" }];
export function gridBindingsOf(code, variant) {
  const kind = SPEC[code].call({ code, variant }, SAMPLE_SIGS).kind;
  return kind === "api" ? bindingsOf(code, variant) : [...bindingsOf(code, variant), ...VALUE_BINDINGS];
}

// ---------------------------------------------------------------------------------------------------------------------
// Printing a `use` node. print.mjs calls `describeUse`; the aux marker lets the printer find the wrapper's inner call line.

export const AUX = "@@AUX@@";

/**
 * @returns {{ text: string, solid: string[], web: string[], namespaces: string[], moduleLines: string[], preludeLines: string[], contextLine?: string }}
 *   `text` is the expression at the site; `solid`/`web` are the import specifiers (`flush`, `flush as flushAlias`); `moduleLines` go after
 *   the imports; `preludeLines` go at the top of the component body; AUX in a line marks the call the runtime frame sits in.
 */
export function describeUse(node, signals) {
  const spec = SPEC[node.code];
  const sigs = (node.sigs ?? []).map((name) => signals.get(name));
  const target = spec.call(node, sigs);
  const out = { text: "", solid: [], web: [], namespaces: [], moduleLines: [], preludeLines: [], contextLine: spec.contextLine };
  const importsOf = (module) => (module === "@solidjs/web" ? out.web : out.solid);
  if (target.kind === "api") {
    const { module, name, args } = target;
    const wrap = (callee) => `${callee}(${args})`;
    switch (node.bind) {
      case "namespace": out.namespaces.push(module); out.text = wrap(`${MODULE[module]}.${name}`); break;
      case "alias": importsOf(module).push(`${name} as ${name}Alias`); out.text = wrap(`${name}Alias`); break;
      case "destructured": out.namespaces.push(module); out.moduleLines.push(`const { ${name}: ${name}D } = ${MODULE[module]};`); out.text = wrap(`${name}D`); break;
      case "wrapper": {
        importsOf(module).push(name);
        if (target.locals) out.preludeLines.push(`const wrapUse = () => { ${AUX}${wrap(name)}; };`);
        else out.moduleLines.push(`function wrapUse() { ${AUX}${wrap(name)}; }`);
        out.text = "wrapUse()";
        break;
      }
      default: importsOf(module).push(name); out.text = wrap(name);
    }
    return out;
  }
  if (target.kind === "local") {
    const { name, args } = target;
    // A value source (`prop-value`): the value is a getter on `props`, so the read is the member itself, not a call.
    const bare = sigs.some((sig) => sig?.read === "value" && sig.name === name);
    switch (node.bind) {
      case "alias": out.preludeLines.push(`const useIt = ${name};`); out.text = `useIt(${args})`; break;
      case "destructured": out.preludeLines.push(`const { useIt } = { useIt: ${name} };`); out.text = `useIt(${args})`; break;
      case "wrapper": out.preludeLines.push(`const wrapUse = () => { ${AUX}${bare ? name : `${name}(${args})`}; };`); out.text = "wrapUse()"; break;
      case "hook": out.preludeLines.push(`const useThing = () => ({ useIt: ${name} });`, "const { useIt } = useThing();"); out.text = `useIt(${args})`; break;
      case "hook-member": out.preludeLines.push(`const useThing = () => ({ useIt: ${name} });`, "const thing = useThing();"); out.text = `thing.useIt(${args})`; break;
      case "tuple": out.preludeLines.push(`const pair = [0, ${name}] as const;`, "const [, useIt] = pair;"); out.text = `useIt(${args})`; break;
      default: out.text = bare ? name : `${name}(${args})`;
    }
    return out;
  }
  // member: a statement on a component-scope object
  switch (node.bind) {
    case "alias": out.preludeLines.push(`const useIt = ${target.object};`); out.text = target.text("useIt"); break;
    case "wrapper": out.preludeLines.push(`const wrapUse = () => { ${AUX}${target.text(target.object)}; };`); out.text = "wrapUse()"; break;
    case "hook": out.preludeLines.push(`const useThing = () => ({ useIt: ${target.object} });`, "const { useIt } = useThing();"); out.text = target.text("useIt"); break;
    case "hook-member": out.preludeLines.push(`const useThing = () => ({ useIt: ${target.object} });`, "const thing = useThing();"); out.text = target.text("thing.useIt"); break;
    case "tuple": out.preludeLines.push(`const pair = [0, ${target.object}] as const;`, "const [, useIt] = pair;"); out.text = target.text("useIt"); break;
    default: out.text = target.text(target.object);
  }
  return out;
}

// ---------------------------------------------------------------------------------------------------------------------
// Building a program

/** Wraps `stmts` in a layer (a closure called in place, a helper taking the callback, an IIFE). */
function applyLayer(layer, stmts, kit) {
  switch (layer) {
    case "closure": { const f = kit.name("f"); return [decl(f, fn(stmts)), expr(call(ref(f)))]; }
    case "helper": return [expr(call(ref(kit.ensureHelper("call")), [fn(stmts)]))];
    case "module-helper": return [expr(call(ref(kit.ensureHelper("call", { module: true })), [fn(stmts)]))];
    case "iife": return [expr(iife(fn(stmts)))];
    default: return stmts;
  }
}

/**
 * Places `stmts` (which hold the site) in a context. Returns { body, view } (the statements of the component body and its JSX items).
 * Contexts that a layer cannot wrap (an async compute needs its await first) take the layers inside.
 */
function inContext(context, stmts, kit) {
  const v = () => kit.name("v");
  const asView = (items) => items;
  switch (context) {
    case "body": return { body: stmts, view: [] };
    case "memo": { const m = kit.name("m"); return { body: [decl(m, prim("memo", fn([...stmts, ret(num(1))])))], view: [child(v(), call(ref(m)))] }; }
    case "effect-compute": return { body: [expr(prim("effect", fn([...stmts, ret(num(1))]), { apply: fn([]) }))], view: [] };
    case "effect-apply": return { body: [expr(prim("effect", fn([ret(num(1))]), { apply: fn(stmts) }))], view: [] };
    case "render-compute": return { body: [expr(prim("render", fn([...stmts, ret(num(1))]), { apply: fn([]) }))], view: [] };
    case "render-apply": return { body: [expr(prim("render", fn([ret(num(1))]), { apply: fn(stmts) }))], view: [] };
    case "effect-apply-guarded": return { body: [expr(prim("effect", fn([ret(num(1))]), { apply: fn([contain("guard-return", 0, stmts)]) }))], view: [] };
    case "effect-apply-guarded-closure": case "render-apply-guarded-closure": {
      const step = kit.name("g");
      const apply = fn([decl(step, fn([contain("guard-return", 0, stmts)])), expr(call(ref(step)))]);
      return { body: [expr(prim(context.startsWith("effect") ? "effect" : "render", fn([ret(num(1))]), { apply }))], view: [] };
    }
    case "effect-apply-guarded-rerun": case "render-apply-guarded-rerun": {
      const trigger = signal(kit.name("t"), 1, "src");
      const apply = fn([contain("guard-return", 0, stmts)]);
      const compute = fn([ret(read(trigger.name))]);
      return {
        body: [expr(prim(context.startsWith("effect") ? "effect" : "render", compute, { apply }))],
        view: [button(kit.name("b"), [expr(write(trigger.name, 2))])],
        sigs: [trigger],
      };
    }
    case "render-apply-guarded": return { body: [expr(prim("render", fn([ret(num(1))]), { apply: fn([contain("guard-return", 0, stmts)]) }))], view: [] };
    case "tracked": return { body: [expr(prim("tracked", fn(stmts)))], view: [] };
    case "settled": return { body: [expr(prim("settled", fn(stmts)))], view: [] };
    case "untrack": return { body: [expr(prim("untrack", fn(stmts)))], view: [] };
    case "root": return { body: [expr(prim("root", fn(stmts)))], view: [] };
    case "owner-null": return { body: [expr(prim("owner", fn(stmts), { ownerArg: "null" }))], view: [] };
    case "timer": return { body: [expr(prim("timer", fn(stmts)))], view: [] };
    case "then": return { body: [expr(prim("then", fn(stmts)))], view: [] };
    case "micro": return { body: [expr(prim("micro", fn(stmts)))], view: [] };
    case "handler": return { body: [], view: [button(kit.name("b"), stmts)] };
    case "async": { const f = kit.name("f"); return { body: [decl(f, fn([awaitStmt(), ...stmts], { async: true })), expr(call(ref(f)))], view: [] }; }
    case "jsx": case "show": case "for": {
      const item = child(v(), iife(fn([...stmts, ret(num(1))])));
      return { body: [], view: asView([context === "show" ? show(num(1), [item]) : context === "for" ? forEach([item]) : item]) };
    }
    case "action": {
      const a = kit.name("a");
      return { body: [decl(a, prim("action", fn(stmts)))], view: [button(kit.name("b"), [expr(call(ref(a)))])] };
    }
    case "memo-async": {
      const m = kit.name("m");
      return { body: [decl(m, prim("memo", fn([awaitStmt(), ...stmts, ret(num(1))], { async: true }))), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))], view: [] };
    }
    case "effect-async": return { body: [expr(prim("effect", fn([awaitStmt(), ...stmts, ret(num(1))], { async: true }), { apply: fn([]) }))], view: [] };
    default: throw new Error(`unknown misuse context ${context}`);
  }
}

/**
 * Builds the misuse component's parts.
 * @param {{ rng: object, name: (prefix: string) => string, ensureHelper: (form: string) => string }} kit
 * @param {{ code?: string, context?: string, binding?: string, layer?: string, variant?: string|null }} plan fixed choices (the rest is drawn)
 * @returns {{ sigs: object[], body: object[], view: object[], shape: string, misuse: object }}
 */
export function buildMisuse(kit, plan = {}) {
  return plan.combo ? buildCombo(kit, plan) : buildOne(kit, plan);
}

/**
 * The extra uses a combination program may take: every legal (code, context) whose template prints no module-scope context line
 * (two would declare `Ctx` twice), except the `-guarded-rerun` contexts, whose rerun is a strict read Solid reports (a defect in the file).
 * An extra is bound directly: the other bindings' prelude names (`useIt`, `wrapUse`) are fixed.
 */
const extraCells = () => LEGAL_CODES().filter((code) => !SPEC[code].contextLine).flatMap((code) => SPEC[code].legal.filter((context) => !context.endsWith("-rerun")).map((context) => ({ code, context })));

/**
 * `plan.combo` (`--combine`): a combination program. The cell's site (marked `primary`, the one the grids judge as the cell's) shares
 * its component with one or two legal uses of other codes (each its own site, in its own legal context) and, half the time, unrelated
 * reactive code (a signal, a memo that reads it, an effect on the memo, a handler that writes the signal, the memo in the JSX). The
 * values of the cell's site come from a source the shape takes, drawn (local, a parent's props, a context, `helpers.ts`), and the
 * statements of the parts are interleaved. Every draw comes from the program's own seed.
 */
function buildCombo(kit, plan) {
  const { rng } = kit;
  const primary = buildOne(kit, { ...plan, combo: undefined, source: plan.source ?? "draw" });
  const marked = primary.body.concat(primary.view).flatMap((node) => [...walk(node)]).find(({ node }) => node.k === "use");
  marked.node.primary = true;
  const pool = extraCells().filter((cell) => cell.code !== primary.misuse.code);
  const extras = Array.from({ length: rng.int(1, 2) }, () => {
    const cell = rng.pick(pool);
    return buildOne(kit, { code: cell.code, context: cell.context, legal: true, binding: "direct", layer: rng.pick(["none", "closure", "helper", "iife"]) });
  });
  const parts = [primary, ...extras];
  if (rng.chance(0.5)) {
    const d = signal(kit.name("d"), 1, "src");
    const m = kit.name("m");
    parts.push({ sigs: [d], body: [decl(m, prim("memo", fn([ret(read(d.name))]))), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))],
      view: [child(kit.name("v"), call(ref(m))), button(kit.name("b"), [expr(write(d.name, 2))])] });
  }
  // Interleaved: each part keeps its own order, the parts take turns in a drawn order.
  const interleave = (lists) => {
    const queues = lists.map((list) => [...list]).filter((list) => list.length);
    const out = [];
    while (queues.length) { const at = rng.int(0, queues.length - 1); out.push(queues[at].shift()); if (!queues[at].length) queues.splice(at, 1); }
    return out;
  };
  const extra = extras.map((part) => `${part.misuse.code}@${part.misuse.context}`);
  return {
    sigs: parts.flatMap((part) => part.sigs), body: interleave(parts.map((part) => part.body)), view: interleave(parts.map((part) => part.view)),
    shape: `${primary.shape}/combo`, misuse: { ...primary.misuse, combo: { extras: extra, decoys: parts.length > extras.length + 1 } },
  };
}

function buildOne(kit, plan = {}) {
  const { rng } = kit;
  const code = plan.code ?? rng.pick(TEMPLATE_CODES);
  const spec = SPEC[code];
  if (!spec || spec.typedAway) throw new Error(`no misuse template for ${code}`);
  // `plan.legal` (legal.mjs): the same call where it is correct Solid, one of the template's `legal` contexts.
  const contexts = plan.legal ? spec.legal ?? [] : spec.contexts;
  const context = plan.context ?? rng.pick(contexts);
  if (!contexts.includes(context)) throw new Error(`${code} has no ${plan.legal ? "legal " : ""}${context} context`);
  // A variant one source carries (`variantSources`) is the grid's, never drawn: every random draw sees the variants it saw before.
  const variant = "variant" in plan && plan.variant !== undefined ? plan.variant : rng.pick((plan.legal ? legalVariantsOf(code) : variantsOf(code)).filter((v) => !spec.variantSources?.[v]));
  const binding = plan.binding ?? rng.pick(bindingsOf(code, variant));
  const layer = plan.layer ?? rng.weighted([[5, "none"], [3, "closure"], [2, "helper"], [1, "iife"], [1, "closure+helper"]]);
  const sigs = spec.setup(variant, kit.name);
  const site = use(code, { variant, bind: binding, sigs: sigs.map((s) => s.name), site: true, witness: null });
  let stmts = [expr(site)];
  for (const l of layer === "none" ? [] : layer.split("+")) stmts = applyLayer(l, stmts, kit);
  const placed = inContext(context, stmts, kit);
  const source = plan.source === "draw" ? rng.pick(sourcesOf(code, variant, binding, context).filter((s) => !GRID_ONLY_SOURCES.includes(s))) : plan.source ?? "local";
  if (!sourcesOf(code, variant, binding, context).includes(source)) throw new Error(`${code}/${variant}/${binding} cannot take source ${source}`);
  // A parent source renders the component from its parent, plainly; the mount axis is not crossed with it.
  const mount = parentSource(source) ? "child" : plan.mount ?? "root";
  if (!MOUNTS.includes(mount)) throw new Error(`unknown mount ${mount}`);
  const sourced = source === "local" ? {} : { source, sourced: sigs.map((sig) => sig.name) };
  return {
    sigs: [...sigs, ...(placed.sigs ?? [])], body: placed.body, view: placed.view,
    shape: `${code}/${context}/${binding}/${layer}${mount === "root" || source !== "local" ? "" : `/${mount}`}${source === "local" ? "" : `/${source}`}`,
    misuse: { code, context, variant, binding, layer, mount, ...sourced },
  };
}
