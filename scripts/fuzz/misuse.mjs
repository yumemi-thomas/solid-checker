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
  button, child, show, forEach, use, contain, ctl, member as memberOf, obj, jsx, createOwnerExpr, loadingView, asyncDepSignal,
} from "./model.mjs";
import { codePolicy, policyOf } from "./codes.mjs";
import { RECALL_MOUNTS } from "./mounts.mjs";

/** The binding forms. `direct` is the baseline; the others change how the call reaches the API, never what it does. */
export const BINDINGS = ["direct", "namespace", "alias", "destructured", "wrapper"];
export const LAYERS = ["none", "closure", "helper", "iife", "closure+helper", "module-helper"];
/**
 * Where the misuse component is mounted: by the entry's own `render` (`root`, every program before the mount axis), or as a child
 * an exported parent renders: plainly (`child`), under `<Show when={true}>` (`show`), or in a one-row `<For>` (`for`). Grid-only
 * (`--mounts`), never drawn: a root program keeps its seed, id and text.
 */
export const BASE_MOUNTS = ["root", "child", "show", "for"];
/** Every mount a plan may name: the base ones and the recall ones (mounts.mjs RECALL_MOUNTS, `--recall` grids only). */
export const MOUNTS = [...BASE_MOUNTS, ...RECALL_MOUNTS];

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
 *            (`useVals()`), destructured or as an object (`vals.setW1`).
 *   async    the async memo of an async template, created in the parent and read in the child: passed as an accessor, as its
 *            value (`a1={a1()}`), by context, or as an accessor under the parent's `<Loading>`.
 * `props` and `context` take the templates whose values are not async (async values are the `async` axis's); `module` takes all.
 */
export const SOURCE_AXES = {
  props: ["prop", "prop-destructured", "prop-merge", "prop-omit", "prop-spread", "prop-value"],
  context: ["context", "context-destructured"],
  module: ["module", "module-hook", "module-hook-member"],
  async: ["async-prop", "async-value", "async-context", "async-loading"],
  // The recall axes (`--recall`): the values through a barrel module that re-exports `helpers.ts` (`lib.ts`), or a hook that is
  // `helpers.ts`'s default export. Not under `module`, so `--sources module` keeps its programs.
  files: ["module-barrel", "module-default-hook"],
};
export const SOURCES = ["local", ...Object.values(SOURCE_AXES).flat()];
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
  return SOURCES.filter((source) => {
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

/**
 * The recall contexts (`--recall` grids only; `SPEC[code].recallContexts`): places the base contexts do not reach, each a shape
 * docs/open-work.md lists as a known miss or a shape real code writes.
 *   owner lifetime  a write in a root made by a named function, an object method, a hook declared with `function`, a stored closure;
 *                   under `runWithOwner` with a bound `createOwner()` result or one created inline; a store write in an effect's
 *                   returned cleanup on a rerun (the compute reads a signal a button sets)
 *   false cleans    the fuzzer's confirmed false cleans (open-work.md): a component handed to a slot callback, a component rendered
 *                   as a statement; an action called from a closure named in an `onCleanup` and handed by name to `createRoot`
 *                   there, in a root disposed at once
 *   async control   the site written before an `await` in a loop and run on its second pass only (after it), in `for await`, in a `finally`
 *                   after an `await`; in an async memo or effect compute (`memo-async-*`, `effect-async-*`), or in an async function
 *                   the body calls (`async-*`)
 *   shared          the site in a closure the context calls and a handler calls too (only one caller is a defect)
 *   rerun           a tracked effect or an effect apply that runs again after a button writes what its compute reads
 *   after yield     an action body after its first `yield`
 */
const OWNER_LIFETIME = ["root-named", "root-method", "root-hook", "root-stored", "owner-bound", "owner-create"];
// Measured in Chrome (2026-10-30): a signal write or an action call in an effect's returned cleanup on a rerun reports nothing on
// rc.17 (a store write there does, so the cleanup runs), and a component rendered in a timer reports nothing from its `onSettled`:
// neither is a misuse context. A write in a cleanup's `createRoot` is exempt (the disposal); an action call there throws.
const FALSE_CLEAN_CONTEXTS = ["comp-slot", "comp-rendered"];
const ASYNC_CONTROL = ["async-loop", "async-for-await", "async-finally"];
/** The contexts whose site sits in a callback handed to a primitive: the callback-form axis can pass it by name. */
export const CALLBACK_CONTEXTS = ["memo", "effect-compute", "effect-apply", "render-compute", "render-apply", "tracked", "settled", "untrack", "root",
  "owner-null", "timer", "then", "micro", "memo-async", "effect-async", "owner-bound", "owner-create", "tracked-rerun", "effect-apply-rerun",
  "memo-async-loop", "memo-async-for-await", "memo-async-finally", "effect-async-loop", "effect-async-for-await", "effect-async-finally"];
/**
 * The callback forms (`--recall`; the `callback` axis): the primitive's callback is a declared name instead of a literal. `const`,
 * a `function` declaration, an alias of a `const` (`const cb = cb0`), a named function expression, a `let`, a member of a local
 * object (`box.cb`), and one `const` handed to two calls of the primitive (`shared`).
 */
export const CALLBACK_FORMS = ["const", "function", "alias", "named", "let", "bundle", "shared"];
/** The recall layers: control flow around the site (model.mjs CTL_FORMS), and the helper imported from `helpers.ts` in other forms. */
export const CONTROL_LAYERS = ["for", "while", "do", "switch", "try-catch", "try-finally", "label", "generator"];
export const FILE_LAYERS = ["module-default", "module-barrel", "module-star", "module-ns"];
export const RECALL_LAYERS = [...CONTROL_LAYERS, ...FILE_LAYERS];
/**
 * The recall bindings. Of an API: destructurings of the namespace other than the base `const { flush: flushD } = S` (nested, with a
 * default, with a rest, `let`, exported, through another `const` holding the namespace), a `const` member alias, and Solid re-exported
 * by a project module (`solid-api.ts`, by name or as a namespace). Of a value (a setter, an accessor, an action, a store): a `let` or
 * `var` alias, a conditional, the result of an identity call, a literal with a spread, a computed key or a getter, a member of a local
 * literal, a destructuring default, a nested pattern, an array slot. Callee forms of either: `.bind(null)`, `.call(null, ...)`, an
 * extracted method, an assignment or `||=` callee.
 */
export const RECALL_API_BINDINGS = ["ns-nested", "ns-default", "ns-rest", "ns-let", "ns-exported", "ns-held", "ns-member", "reexport", "reexport-ns"];
export const RECALL_VALUE_BINDINGS = ["let-alias", "var-alias", "cond-alias", "identity", "spread-literal", "computed-key", "getter", "literal-member", "default-slot", "nested", "array-slot"];
export const CALLEE_FORMS = ["bind", "call", "extracted", "assign-callee", "or-assign"];

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
  REACTIVE_WRITE_IN_OWNED_SCOPE: { legal: ["handler", "timer", "then", "micro", "effect-apply"], recallContexts: [...OWNER_LIFETIME, ...FALSE_CLEAN_CONTEXTS, "memo-shared"],
    variants: ["signal", "store", "refresh"], contexts: OWNED_PURE, bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => [variant === "store" ? storeSignal(name("w"), 0) : variant === "refresh" ? memoSignal(name("m")) : signal(name("w"), 0, "probe")],
    call: (node, [sig]) => (node.variant === "store" ? local(sig.setter, "(s) => { s.v = 7; }") : node.variant === "refresh" ? api("refresh", sig.name, { locals: true }) : local(sig.setter, "7")),
    // refresh is an API; the other two are local functions, which have no namespace form.
    bindingsFor: (variant) => (variant === "refresh" ? BINDINGS : ["direct", "alias", "destructured", "wrapper"]),
  },
  STORE_WRITE_OUTSIDE_SETTER: { legal: ["handler", "timer", "then"], recallContexts: [...OWNER_LIFETIME, "effect-cleanup-rerun", ...ASYNC_CONTROL],
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
  STRICT_READ_UNTRACKED: { legal: ["memo", "effect-compute", "render-compute", "jsx", "untrack"],
    variants: ["signal", "memo"], contexts: ["body", "effect-apply", "root", "render-apply"], bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => [variant === "memo" ? memoSignal(name("m")) : signal(name("r"), 3, "src")],
    call: (node, [sig]) => local(sig.name),
  },
  NO_OWNER_CLEANUP: { legal: ["body", "memo", "effect-compute", "render-compute", "root"], recallContexts: ASYNC_CONTROL, contexts: UNOWNED, bindings: BINDINGS, setup: () => [], call: () => api("onCleanup", "() => {}") },
  NO_OWNER_EFFECT: { legal: ["body", "root"], recallContexts: ASYNC_CONTROL,
    variants: ["effect", "render"], contexts: UNOWNED, bindings: BINDINGS, setup: () => [],
    call: (node) => api(node.variant === "render" ? "createRenderEffect" : "createEffect", "() => 1, () => {}"),
  },
  NO_OWNER_BOUNDARY: { recallContexts: ASYNC_CONTROL,
    variants: ["loading", "errored"], contexts: UNOWNED, bindings: BINDINGS, setup: () => [],
    call: (node) => api(node.variant === "errored" ? "Errored" : "Loading", "{ fallback: 0, children: 1 }"),
  },
  NO_OWNER_CONTEXT: { legal: ["body", "memo", "root"], recallContexts: ASYNC_CONTROL, contexts: UNOWNED, bindings: BINDINGS, setup: () => [], contextLine: "const Ctx = createContext(7);", call: () => api("useContext", "Ctx") },
  CONTEXT_NOT_FOUND: {
    contexts: ["body", "memo", "render-compute", "root", "jsx", "show", "for"], bindings: BINDINGS, setup: () => [],
    contextLine: "const Ctx = createContext<number>();", call: () => api("useContext", "Ctx"),
  },
  SETTLED_CLEANUP_UNOWNED: { legal: ["body", "root"], recallContexts: ASYNC_CONTROL, contexts: [...UNOWNED, "effect-apply"], bindings: BINDINGS, setup: () => [], call: () => api("onSettled", "() => () => {}") },
  CLEANUP_IN_FORBIDDEN_SCOPE: { legal: ["body", "root"], recallContexts: ["tracked-rerun"], contexts: LEAF, bindings: BINDINGS, setup: () => [], call: () => api("onCleanup", "() => {}") },
  PRIMITIVE_IN_FORBIDDEN_SCOPE: { legal: ["body", "root"], recallContexts: ["tracked-rerun"],
    variants: ["memo", "root", "effect"], contexts: LEAF, bindings: BINDINGS, setup: () => [],
    call: (node) => (node.variant === "root" ? api("createRoot", "() => 1") : node.variant === "effect" ? api("createEffect", "() => 1, () => {}") : api("createMemo", "() => 1")),
  },
  ASYNC_OUTSIDE_LOADING_BOUNDARY: {
    contexts: ["jsx", "show", "for"], bindings: ["direct", "alias", "destructured", "wrapper"], placeless: true,
    setup: (variant, name) => [asyncSignal(name("a"))], call: (node, [sig]) => local(sig.name),
  },
  // rc.17 dev-shared.js: a pending read inside createTrackedEffect or onSettled serves the committed value (and throws while uninitialized).
  PENDING_ASYNC_FORBIDDEN_SCOPE: { recallContexts: ["tracked-rerun"],
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
  UNTRACKED_READ_AFTER_AWAIT: { recallContexts: ["memo-async-loop", "memo-async-for-await", "memo-async-finally", "effect-async-loop", "effect-async-for-await", "effect-async-finally"],
    contexts: ["memo-async", "effect-async"], bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => [signal(name("r"), 3, "src")], call: (node, [sig]) => local(sig.name),
  },
  // Defect codes no rule claims (codes.mjs `defect-unclaimed`): measured for "never quieter than Solid"; a silent checker is a false clean.
  // rc.17 dev-shared.js `assertNotInEffectCallback`: a memo, effect or root created with no owner in an effect callback belongs to nobody.
  PRIMITIVE_IN_EFFECT_CALLBACK: { legal: ["body", "root"], recallContexts: ["effect-apply-rerun"],
    // Effect apply only: in a render effect's apply rc.17 does not report it (the fast path fires it in every effect-apply cell).
    variants: ["memo", "root", "effect"], contexts: ["effect-apply"], bindings: BINDINGS, setup: () => [],
    call: (node) => (node.variant === "root" ? api("createRoot", "() => 1") : node.variant === "effect" ? api("createEffect", "() => 1, () => {}") : api("createMemo", "() => 1")),
  },
  // rc.17 signals dev.js: a `<Loading on>` re-armed by a write while a source it waits on is also read outside it (the fallback can
  // never show). Unclaimed: measured for "never quieter than Solid". The site is the outside read; the diagnostic is raised from the
  // boundary's notification (no authored frame is guaranteed), so it is judged by file. Recall grids only (`recallOnly`).
  LOADING_ON_OUTSIDE_HOLD: { recallOnly: true, placeless: true,
    contexts: [], recallContexts: ["loading-on-outside"], bindings: ["direct", "alias", "wrapper"],
    setup: (variant, name) => { const trigger = signal(name("t"), 1, "src"); return [trigger, asyncDepSignal(name("a"), trigger.name)]; },
    call: (node, [, sig]) => local(sig.name),
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
  ACTION_CALLED_IN_OWNED_SCOPE: { legal: ["handler", "timer", "then", "micro"], recallContexts: [...OWNER_LIFETIME, ...FALSE_CLEAN_CONTEXTS, "cleanup-root-named", "memo-shared"],
    contexts: OWNED_PURE, bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => { const target = signal(name("w"), 0, "probe"); return [target, actionSignal(name("a"), target.name, 7)]; },
    call: (node, [, act]) => local(act.name),
  },
  RESOLVE_IN_REACTIVE_SCOPE: { legal: ["handler", "timer"], recallContexts: ["memo-shared"], contexts: ["memo", "render-compute"], bindings: BINDINGS, setup: () => [], call: () => api("resolve", "() => 1") },
  UNTIL_IN_REACTIVE_SCOPE: { legal: ["handler", "timer"], recallContexts: ["memo-shared"], contexts: ["memo", "render-compute"], bindings: BINDINGS, setup: () => [], call: () => api("until", "() => 1") },
  FLUSH_IN_ACTION: { legal: ["handler", "timer"], recallContexts: ["action-after-yield", "action-shared"], contexts: ["action"], bindings: BINDINGS, setup: () => [], call: () => api("flush") },
  FLUSH_IN_EFFECT_CALLBACK: { legal: ["handler", "timer"], recallContexts: ["effect-apply-rerun", "effect-apply-shared"], contexts: ["effect-apply"], bindings: BINDINGS, setup: () => [], call: () => api("flush") },
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: { legal: ["handler", "timer"], recallContexts: ["tracked-rerun", "tracked-shared"], contexts: LEAF, bindings: BINDINGS, setup: () => [], call: () => api("flush") },
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
export const TEMPLATE_CODES = MISUSE_CODES.filter((code) => !SPEC[code].typedAway && !SPEC[code].recallOnly);
/** The codes with a template only the recall grids measure (never drawn, so no other program moves). */
export const RECALL_TEMPLATE_CODES = MISUSE_CODES.filter((code) => SPEC[code].recallOnly);
export const contextsOf = (code) => SPEC[code].contexts;
/** The recall contexts of a code (`--recall` grids only). */
export const recallContextsOf = (code) => SPEC[code].recallContexts ?? [];
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
/** The recall bindings a (code, variant) takes: by its call's kind; a statement on a store (`member`) has no callee to rewrite. */
export function recallBindingsOf(code, variant) {
  const kind = SPEC[code].call({ code, variant }, SAMPLE_SIGS).kind;
  return kind === "api" ? [...RECALL_API_BINDINGS, ...CALLEE_FORMS] : kind === "local" ? [...RECALL_VALUE_BINDINGS, ...CALLEE_FORMS] : RECALL_VALUE_BINDINGS;
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
    const ns = MODULE[module];
    const local = `${name}N`;
    switch (node.bind) {
      // The recall bindings (RECALL_API_BINDINGS, CALLEE_FORMS).
      case "ns-nested": out.namespaces.push(module); out.moduleLines.push(`const { inner: { ${name}: ${local} } } = { inner: ${ns} };`); out.text = wrap(local); break;
      case "ns-default": out.namespaces.push(module); out.moduleLines.push(`const { ${name}: ${local} = ${ns}.${name} } = ${ns};`); out.text = wrap(local); break;
      case "ns-rest": out.namespaces.push(module); out.moduleLines.push(`const { ${name}: ${local}, ...${name}Rest } = ${ns};`); out.text = wrap(local); break;
      case "ns-let": out.namespaces.push(module); out.moduleLines.push(`let { ${name}: ${local} } = ${ns};`); out.text = wrap(local); break;
      case "ns-exported": out.namespaces.push(module); out.moduleLines.push(`export const { ${name}: ${local} } = ${ns};`); out.text = wrap(local); break;
      case "ns-held": out.namespaces.push(module); out.moduleLines.push(`const ${ns}2 = ${ns};`, `const { ${name}: ${local} } = ${ns}2;`); out.text = wrap(local); break;
      case "ns-member": out.namespaces.push(module); out.moduleLines.push(`const ${local} = ${ns}.${name};`); out.text = wrap(local); break;
      case "reexport": out.importLines = [`import { ${name} as ${local} } from "./solid-api";`]; out.files = { "solid-api.ts": `export { ${name} } from "${module}";\n` }; out.text = wrap(local); break;
      case "reexport-ns": out.importLines = ['import * as R from "./solid-api";']; out.files = { "solid-api.ts": `export * from "${module}";\n` }; out.text = wrap(`R.${name}`); break;
      case "bind": importsOf(module).push(name); out.text = `${name}.bind(null)(${args})`; break;
      case "call": importsOf(module).push(name); out.text = `${name}.call(null${args ? `, ${args}` : ""})`; break;
      case "extracted": importsOf(module).push(name); out.moduleLines.push(`const ${name}Box = { m: ${name} };`, `const ${local} = ${name}Box.m;`); out.text = wrap(local); break;
      case "assign-callee": importsOf(module).push(name); out.moduleLines.push(`let ${name}Slot: typeof ${name} | undefined;`); out.text = `(${name}Slot = ${name})(${args})`; break;
      case "or-assign": importsOf(module).push(name); out.moduleLines.push(`let ${name}Slot: typeof ${name} | undefined;`); out.text = `(${name}Slot ||= ${name})(${args})`; break;
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
      case "bind": out.text = `${name}.bind(null)(${args})`; break;
      case "call": out.text = `${name}.call(null${args ? `, ${args}` : ""})`; break;
      case "extracted": out.preludeLines.push(`const box = { m: ${name} };`, "const useIt = box.m;"); out.text = `useIt(${args})`; break;
      case "assign-callee": out.preludeLines.push(`let slot: typeof ${name} | undefined;`); out.text = `(slot = ${name})(${args})`; break;
      case "or-assign": out.preludeLines.push(`let slot: typeof ${name} | undefined;`); out.text = `(slot ||= ${name})(${args})`; break;
      default: {
        const lines = valueBinding(node.bind, name);
        if (lines) { out.preludeLines.push(...lines.prelude); out.text = `${lines.callee}(${args})`; break; }
        out.text = bare ? name : `${name}(${args})`;
      }
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
    default: {
      const lines = valueBinding(node.bind, target.object);
      if (lines) { out.preludeLines.push(...lines.prelude); out.text = target.text(lines.callee); break; }
      out.text = target.text(target.object);
    }
  }
  return out;
}

/** A recall value binding (RECALL_VALUE_BINDINGS) of `name`: the component prelude lines and the name the site uses; null otherwise. */
function valueBinding(bind, name) {
  switch (bind) {
    case "let-alias": return { prelude: [`let useIt = ${name};`], callee: "useIt" };
    case "var-alias": return { prelude: [`var useIt = ${name};`], callee: "useIt" };
    // Both arms are the value: the conditional is the shape, not a choice.
    case "cond-alias": return { prelude: [`const useIt = Math.random() < 2 ? ${name} : ${name};`], callee: "useIt" };
    case "identity": return { prelude: [`const useIt = ((x: typeof ${name}) => x)(${name});`], callee: "useIt" };
    case "spread-literal": return { prelude: [`const { useIt } = { ...{ useIt: ${name} } };`], callee: "useIt" };
    case "computed-key": return { prelude: [`const { useIt } = { ["useIt"]: ${name} };`], callee: "useIt" };
    case "getter": return { prelude: [`const { useIt } = { get useIt() { return ${name}; } };`], callee: "useIt" };
    case "literal-member": return { prelude: [`const box = { useIt: ${name} };`], callee: "box.useIt" };
    case "default-slot": return { prelude: [`const { useIt = ${name} } = {} as { useIt?: typeof ${name} };`], callee: "useIt" };
    case "nested": return { prelude: [`const { a: { useIt } } = { a: { useIt: ${name} } };`], callee: "useIt" };
    case "array-slot": return { prelude: [`const [useIt] = [${name}];`], callee: "useIt" };
    default: return null;
  }
}

// ---------------------------------------------------------------------------------------------------------------------
// Building a program

/** Wraps `stmts` in a layer (a closure called in place, a helper taking the callback, an IIFE). */
function applyLayer(layer, stmts, kit) {
  if (CONTROL_LAYERS.includes(layer)) return [ctl(layer, stmts)];
  if (FILE_LAYERS.includes(layer)) return [expr(call(ref(kit.ensureHelper("call", { module: true, via: layer.slice("module-".length) })), [fn(stmts)]))];
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
    default: return recallContext(context, stmts, kit);
  }
}

/** The recall contexts (see OWNER_LIFETIME above). */
function recallContext(context, stmts, kit) {
  const awaited = { loop: "loop-await", "for-await": "for-await", finally: "finally-await" };
  const trigger = () => signal(kit.name("t"), 1, "src");
  const after = /^(memo-async|effect-async|async)-(loop|for-await|finally)$/.exec(context);
  if (after) {
    const body = [ctl(awaited[after[2]], stmts)];
    if (after[1] === "memo-async") {
      const m = kit.name("m");
      return { body: [decl(m, prim("memo", fn([...body, ret(num(1))], { async: true }))), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))], view: [] };
    }
    if (after[1] === "effect-async") return { body: [expr(prim("effect", fn([...body, ret(num(1))], { async: true }), { apply: fn([]) }))], view: [] };
    const f = kit.name("f");
    return { body: [decl(f, fn(body, { async: true })), expr(call(ref(f)))], view: [] };
  }
  const rooted = () => expr(prim("root", fn(stmts)));
  switch (context) {
    case "root-named": { const f = kit.name("f"); return { body: [decl(f, fn([rooted()])), expr(call(ref(f)))], view: [] }; }
    case "root-hook": { const h = kit.name("useHook"); return { body: [decl(h, fn([rooted()], { form: "function" })), expr(call(ref(h)))], view: [] }; }
    case "root-method": { const o = kit.name("o"); return { body: [decl(o, obj([["m", fn([rooted()], { form: "method" })]])), expr(call(memberOf(ref(o), "m")))], view: [] }; }
    case "root-stored": { const o = kit.name("o"); return { body: [decl(o, obj([["f", fn([rooted()])]])), expr(call(memberOf(ref(o), "f")))], view: [] }; }
    case "owner-bound": return { body: [decl("boundOwner", createOwnerExpr()), expr(prim("owner", fn(stmts), { ownerArg: "bound" }))], view: [] };
    case "owner-create": return { body: [expr(prim("owner", fn(stmts), { ownerArg: "create" }))], view: [] };
    case "effect-cleanup-rerun": {
      const t = trigger();
      return { body: [expr(prim("effect", fn([ret(read(t.name))]), { apply: fn([ret(fn(stmts))]) }))], view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t] };
    }
    case "comp-slot": return { body: [contain("comp-slot", 9, [...stmts, ret(num(1))])], view: [] };
    case "comp-rendered": return { body: [contain("comp-rendered", 9, [...stmts, ret(jsx())])], view: [] };
    case "timer-comp-settled": return { body: [expr(prim("timer", fn([contain("comp-rendered", 9, [expr(prim("settled", fn(stmts))), ret(jsx())])])))], view: [] };
    case "cleanup-root-named": {
      const g = kit.name("g");
      const inner = [decl(g, fn(stmts)), expr(prim("cleanup", fn([expr(prim("root", ref(g)))]))), expr(call(ref("dispose"))), ret(ref("dispose"))];
      return { body: [expr(prim("rootd", fn(inner)))], view: [] };
    }
    case "memo-shared": case "effect-apply-shared": case "action-shared": case "tracked-shared": {
      const g = kit.name("g");
      const calls = [expr(call(ref(g)))];
      const caller = button(kit.name("b"), [expr(call(ref(g)))]);
      if (context === "memo-shared") { const m = kit.name("m"); return { body: [decl(g, fn(stmts)), decl(m, prim("memo", fn([...calls, ret(num(1))])))], view: [child(kit.name("v"), call(ref(m))), caller] }; }
      if (context === "effect-apply-shared") return { body: [decl(g, fn(stmts)), expr(prim("effect", fn([ret(num(1))]), { apply: fn(calls) }))], view: [caller] };
      if (context === "tracked-shared") return { body: [decl(g, fn(stmts)), expr(prim("tracked", fn(calls)))], view: [caller] };
      const a = kit.name("a");
      return { body: [decl(g, fn(stmts)), decl(a, prim("action", fn(calls)))], view: [button(kit.name("b"), [expr(call(ref(a)))]), caller] };
    }
    case "action-after-yield": { const a = kit.name("a"); return { body: [decl(a, prim("action", fn(stmts), { afterYield: true }))], view: [button(kit.name("b"), [expr(call(ref(a)))])] }; }
    case "tracked-rerun": {
      const t = trigger();
      return { body: [expr(prim("tracked", fn([expr(read(t.name)), ...stmts])))], view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t] };
    }
    case "effect-apply-rerun": {
      const t = trigger();
      return { body: [expr(prim("effect", fn([ret(read(t.name))]), { apply: fn(stmts) }))], view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t] };
    }
    // LOADING_ON_OUTSIDE_HOLD: the async memo (setup) reads the trigger; `<Loading on={t()}>` holds a read of it, the site reads it
    // outside, and a button writes the trigger, which re-arms the boundary while the outside read holds the frame.
    case "loading-on-outside": {
      const item = child(kit.name("v"), iife(fn([...stmts, ret(num(1))])));
      return { body: [], view: [item], loading: true };
    }
    default: throw new Error(`unknown misuse context ${context}`);
  }
}

/**
 * The callback-form axis (CALLBACK_FORMS): the callback of the context's primitive (its apply in an `-apply` context) becomes a
 * declared name, declared just before the statement that hands it on.
 */
function nameCallback(placed, context, form, kit) {
  const at = placed.body.findIndex((stmt) => (stmt.k === "expr" || stmt.k === "decl") && stmt.e.k === "prim");
  if (at < 0) throw new Error(`${context} has no primitive to hand a callback to`);
  const host = placed.body[at].e;
  const key = /apply/.test(context) ? "apply" : "fn";
  const callback = host[key];
  if (callback?.k !== "fn") throw new Error(`${context}: the ${key} is not a function literal`);
  const cb = kit.name("cb");
  const decls = [];
  switch (form) {
    case "const": decls.push(decl(cb, callback)); host[key] = ref(cb); break;
    case "let": decls.push(decl(cb, callback, { kw: "let" })); host[key] = ref(cb); break;
    case "function": decls.push(decl(cb, { ...callback, form: "function" })); host[key] = ref(cb); break;
    case "named": decls.push(decl(cb, { ...callback, form: "named", label: `${cb}Named` })); host[key] = ref(cb); break;
    case "alias": { const first = kit.name("cb"); decls.push(decl(first, callback), decl(cb, ref(first))); host[key] = ref(cb); break; }
    case "bundle": decls.push(decl(cb, obj([["cb", callback]]))); host[key] = memberOf(ref(cb), "cb"); break;
    case "shared": {
      decls.push(decl(cb, callback));
      host[key] = ref(cb);
      placed.body.splice(at + 1, 0, expr(structuredClone(host)));
      break;
    }
    default: throw new Error(`unknown callback form ${form}`);
  }
  placed.body.splice(at, 0, ...decls);
  return placed;
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
  const recall = plan.legal ? [] : recallContextsOf(code);
  if (!contexts.includes(context) && !recall.includes(context)) throw new Error(`${code} has no ${plan.legal ? "legal " : ""}${context} context`);
  const variant = "variant" in plan && plan.variant !== undefined ? plan.variant : rng.pick(plan.legal ? legalVariantsOf(code) : variantsOf(code));
  const binding = plan.binding ?? rng.pick(bindingsOf(code, variant));
  const layer = plan.layer ?? rng.weighted([[5, "none"], [3, "closure"], [2, "helper"], [1, "iife"], [1, "closure+helper"]]);
  const sigs = spec.setup(variant, kit.name);
  const site = use(code, { variant, bind: binding, sigs: sigs.map((s) => s.name), site: true, witness: null });
  let stmts = [expr(site)];
  for (const l of layer === "none" ? [] : layer.split("+")) stmts = applyLayer(l, stmts, kit);
  const placed = inContext(context, stmts, kit);
  if (plan.callback) {
    if (!CALLBACK_CONTEXTS.includes(context)) throw new Error(`${context} takes no callback form`);
    nameCallback(placed, context, plan.callback, kit);
  }
  const source = plan.source === "draw" ? rng.pick(sourcesOf(code, variant, binding, context)) : plan.source ?? "local";
  if (!sourcesOf(code, variant, binding, context).includes(source)) throw new Error(`${code}/${variant}/${binding} cannot take source ${source}`);
  // A parent source renders the component from its parent, plainly; the mount axis is not crossed with it.
  const mount = parentSource(source) ? "child" : plan.mount ?? "root";
  if (!MOUNTS.includes(mount)) throw new Error(`unknown mount ${mount}`);
  const sourced = source === "local" ? {} : { source, sourced: sigs.map((sig) => sig.name) };
  // LOADING_ON_OUTSIDE_HOLD's boundary: `<Loading on={t()}>` around a read of the async memo, and the button that writes the trigger.
  const view = placed.loading ? [loadingView(sigs[0].name, [child(kit.name("v"), read(sigs[1].name))]), ...placed.view, button(kit.name("b"), [expr(write(sigs[0].name, 2))])] : placed.view;
  return {
    sigs: [...sigs, ...(placed.sigs ?? [])], body: placed.body, view,
    shape: `${code}/${context}/${binding}/${layer}${mount === "root" || source !== "local" ? "" : `/${mount}`}${source === "local" ? "" : `/${source}`}${plan.callback ? `/cb:${plan.callback}` : ""}`,
    misuse: { code, context, variant, binding, layer, mount, ...sourced, ...(plan.callback ? { callback: plan.callback } : {}) },
  };
}
