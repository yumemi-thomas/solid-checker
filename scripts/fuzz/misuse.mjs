// The misuse profile: one program per Solid dev diagnostic code that a rule claims, built to provoke exactly that code at one marked
// site (a `use` node), in a chosen context, through a chosen binding form.
//
//   code      a key of SPEC (the claimed codes of the policy table, codes.mjs)
//   context   where the call sits: a component body, a memo or effect compute, an effect or render-effect apply, createTrackedEffect,
//             onSettled, untrack, createRoot, runWithOwner(null), a handler, a timer, a promise callback, a microtask, after an await,
//             a JSX child (plain, under Show, in a For row), an action body, an async memo or effect compute
//   layer     none, a closure (named, called), a same-file helper taking the callback, an IIFE, or two of them
//   binding   the call reaches its API through a direct import, a namespace import, an alias, a destructuring, or a wrapper function
//
// A cell of the recall matrix (matrix.mjs) is (code, context). `SPEC[code].contexts` lists the contexts the template is meant to fire
// in; a context where the call is legal (a write in a handler) is not in the list. `typedAway` codes have no template: the published
// typings reject every program that could provoke them, so `tsc` already reports it (invariant 1) and the checker stays silent.
import {
  signal, storeSignal, memoSignal, actionSignal, asyncSignal, storeOptSignal, num, ref, call, iife, fn, prim, expr, decl, ret, awaitStmt,
  button, child, show, forEach, use,
} from "./model.mjs";
import { codePolicy, policyOf } from "./codes.mjs";

/** The binding forms. `direct` is the baseline; the others change how the call reaches the API, never what it does. */
export const BINDINGS = ["direct", "namespace", "alias", "destructured", "wrapper"];
export const LAYERS = ["none", "closure", "helper", "iife", "closure+helper"];

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
  REACTIVE_WRITE_IN_OWNED_SCOPE: {
    variants: ["signal", "store", "refresh"], contexts: OWNED_PURE, bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => [variant === "store" ? storeSignal(name("w"), 0) : variant === "refresh" ? memoSignal(name("m")) : signal(name("w"), 0, "probe")],
    call: (node, [sig]) => (node.variant === "store" ? local(sig.setter, "(s) => { s.v = 7; }") : node.variant === "refresh" ? api("refresh", sig.name, { locals: true }) : local(sig.setter, "7")),
    // refresh is an API; the other two are local functions, which have no namespace form.
    bindingsFor: (variant) => (variant === "refresh" ? BINDINGS : ["direct", "alias", "destructured", "wrapper"]),
  },
  STORE_WRITE_OUTSIDE_SETTER: {
    variants: ["assign", "delete"], contexts: [...OWNED_PURE.filter((c) => !["show", "for"].includes(c)), "effect-apply", "handler", "timer", "then", "async"],
    bindings: ["direct", "alias", "wrapper"],
    setup: (variant, name) => [variant === "delete" ? storeOptSignal(name("s"), 0) : storeSignal(name("s"), 0)],
    call: (node, [sig]) => member(sig.name, (object) => (node.variant === "delete" ? `delete ${object}.v` : `${object}.v = 7`)),
  },
  STRICT_READ_UNTRACKED: {
    variants: ["signal", "memo"], contexts: ["body", "effect-apply", "root", "render-apply"], bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => [variant === "memo" ? memoSignal(name("m")) : signal(name("r"), 3, "src")],
    call: (node, [sig]) => local(sig.name),
  },
  NO_OWNER_CLEANUP: { contexts: UNOWNED, bindings: BINDINGS, setup: () => [], call: () => api("onCleanup", "() => {}") },
  NO_OWNER_EFFECT: {
    variants: ["effect", "render"], contexts: UNOWNED, bindings: BINDINGS, setup: () => [],
    call: (node) => api(node.variant === "render" ? "createRenderEffect" : "createEffect", "() => 1, () => {}"),
  },
  NO_OWNER_BOUNDARY: {
    variants: ["loading", "errored"], contexts: UNOWNED, bindings: BINDINGS, setup: () => [],
    call: (node) => api(node.variant === "errored" ? "Errored" : "Loading", "{ fallback: 0, children: 1 }"),
  },
  NO_OWNER_CONTEXT: { contexts: UNOWNED, bindings: BINDINGS, setup: () => [], contextLine: "const Ctx = createContext(7);", call: () => api("useContext", "Ctx") },
  CONTEXT_NOT_FOUND: {
    contexts: ["body", "memo", "render-compute", "root", "jsx", "show", "for"], bindings: BINDINGS, setup: () => [],
    contextLine: "const Ctx = createContext<number>();", call: () => api("useContext", "Ctx"),
  },
  SETTLED_CLEANUP_UNOWNED: { contexts: [...UNOWNED, "effect-apply"], bindings: BINDINGS, setup: () => [], call: () => api("onSettled", "() => () => {}") },
  CLEANUP_IN_FORBIDDEN_SCOPE: { contexts: LEAF, bindings: BINDINGS, setup: () => [], call: () => api("onCleanup", "() => {}") },
  PRIMITIVE_IN_FORBIDDEN_SCOPE: {
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
  PENDING_ASYNC_UNTRACKED_READ: {
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
  PRIMITIVE_IN_EFFECT_CALLBACK: {
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
  ACTION_CALLED_IN_OWNED_SCOPE: {
    contexts: OWNED_PURE, bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => { const target = signal(name("w"), 0, "probe"); return [target, actionSignal(name("a"), target.name, 7)]; },
    call: (node, [, act]) => local(act.name),
  },
  RESOLVE_IN_REACTIVE_SCOPE: { contexts: ["memo", "render-compute"], bindings: BINDINGS, setup: () => [], call: () => api("resolve", "() => 1") },
  UNTIL_IN_REACTIVE_SCOPE: { contexts: ["memo", "render-compute"], bindings: BINDINGS, setup: () => [], call: () => api("until", "() => 1") },
  FLUSH_IN_ACTION: { contexts: ["action"], bindings: BINDINGS, setup: () => [], call: () => api("flush") },
  FLUSH_IN_EFFECT_CALLBACK: { contexts: ["effect-apply"], bindings: BINDINGS, setup: () => [], call: () => api("flush") },
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: { contexts: LEAF, bindings: BINDINGS, setup: () => [], call: () => api("flush") },
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
    switch (node.bind) {
      case "alias": out.preludeLines.push(`const useIt = ${name};`); out.text = `useIt(${args})`; break;
      case "destructured": out.preludeLines.push(`const { useIt } = { useIt: ${name} };`); out.text = `useIt(${args})`; break;
      case "wrapper": out.preludeLines.push(`const wrapUse = () => { ${AUX}${name}(${args}); };`); out.text = "wrapUse()"; break;
      case "hook": out.preludeLines.push(`const useThing = () => ({ useIt: ${name} });`, "const { useIt } = useThing();"); out.text = `useIt(${args})`; break;
      case "hook-member": out.preludeLines.push(`const useThing = () => ({ useIt: ${name} });`, "const thing = useThing();"); out.text = `thing.useIt(${args})`; break;
      case "tuple": out.preludeLines.push(`const pair = [0, ${name}] as const;`, "const [, useIt] = pair;"); out.text = `useIt(${args})`; break;
      default: out.text = `${name}(${args})`;
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
  const { rng } = kit;
  const code = plan.code ?? rng.pick(TEMPLATE_CODES);
  const spec = SPEC[code];
  if (!spec || spec.typedAway) throw new Error(`no misuse template for ${code}`);
  const context = plan.context ?? rng.pick(spec.contexts);
  if (!spec.contexts.includes(context)) throw new Error(`${code} has no ${context} context`);
  const variant = "variant" in plan && plan.variant !== undefined ? plan.variant : rng.pick(variantsOf(code));
  const binding = plan.binding ?? rng.pick(bindingsOf(code, variant));
  const layer = plan.layer ?? rng.weighted([[5, "none"], [3, "closure"], [2, "helper"], [1, "iife"], [1, "closure+helper"]]);
  const sigs = spec.setup(variant, kit.name);
  const site = use(code, { variant, bind: binding, sigs: sigs.map((s) => s.name), site: true, witness: null });
  let stmts = [expr(site)];
  for (const l of layer === "none" ? [] : layer.split("+")) stmts = applyLayer(l, stmts, kit);
  const placed = inContext(context, stmts, kit);
  return { sigs, body: placed.body, view: placed.view, shape: `${code}/${context}/${binding}/${layer}`, misuse: { code, context, variant, binding, layer } };
}
