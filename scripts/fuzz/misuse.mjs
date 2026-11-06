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
  walk, domExpr, listenExpr, signal, storeSignal, memoSignal, actionSignal, reactionSignal, asyncSignal, asyncDeclaredSignal, asyncMaybeSignal, storeAsyncSignal, storeRawSignal, storeClassSignal, storeOptSignal, storeNestedSignal, storeListSignal, storeLinkSignal, storeArraySignal, storeEmptySignal, storeShallowSignal, apiSignal, num, ref, call, iife, fn, prim, expr, decl, ret, awaitStmt, read, write,
  HANDOFF_FORMS, DEPTH_FORMS, CHAIN_FORMS, propEl, jsxExpr, button, child, attr, iter, show, forEach, refEl, flowEl, handlerExpr, FLOW_FORMS, flush, use, contain, ctl, member as memberOf, obj, jsx, createOwnerExpr, loadingView, asyncDepSignal,
  CBP_CALLS, CBP_FORMS, cbProp, namedButton,
} from "./model.mjs";
import { codePolicy, policyOf } from "./codes.mjs";
import { RECALL_MOUNTS } from "./mounts.mjs";
import { FLIP_READ_CONTEXTS, FLIP_READ_LEGAL, FLIP_READ_VARIANTS, STATIC_SOURCES, STORE_DESTRUCTURES, SYNC_COMPUTES, flipReadContext, openSignal } from "./flip-read.mjs";
import { FLIP_WRITE_CONTEXTS, flipWriteContext } from "./flip-write.mjs";

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
 *            (`useVals()`), destructured or as an object (`vals.setW1`); or App.tsx holds one module-scope result of the hook
 *            (`shared.w1`) beside the body's own call of it (`module-hook-shared`, the ADR 0511 review's false positive: a read
 *            through another instance), its async memos settled before the mount; or `useVals()` returns the creation itself
 *            (`module-hook-direct`: `return createMemo(...)`, no binding in the hook, ADR 0628).
 *   async    the async memo of an async template, created in the parent and read in the child: passed as an accessor, as its
 *            value (`a1={a1()}`), by context, or as an accessor under the parent's `<Loading>`.
 * `props` and `context` take the templates whose values are not async (async values are the `async` axis's); `module` takes all.
 */
export const SOURCE_AXES = {
  props: ["prop", "prop-destructured", "prop-merge", "prop-omit", "prop-spread", "prop-value"],
  context: ["context", "context-destructured"],
  module: ["module", "module-hook", "module-hook-member", "module-hook-shared", "module-hook-direct"],
  async: ["async-prop", "async-value", "async-value-member", "async-value-dstr", "async-context", "async-loading"],
  // The recall axes (`--recall`): the values through a barrel module that re-exports `helpers.ts` (`lib.ts`), or a hook that is
  // `helpers.ts`'s default export. Not under `module`, so `--sources module` keeps its programs.
  files: ["module-barrel", "module-default-hook"],
};
export const SOURCES = ["local", ...Object.values(SOURCE_AXES).flat()];
/** Sources the grids enumerate and a random draw never takes (as `gridVariants`): random programs and combinations keep theirs. */
export const GRID_ONLY_SOURCES = ["module-hook-shared", "module-hook-direct"];
/** A source whose values a parent component creates (the component is then rendered by `P1`, not exported). */
export const parentSource = (source) => source !== "local" && !source.startsWith("module");
/** A source that hands the component a read's value, not the accessor: only a read written as the misuse itself can use it. */
const valueSource = (source) => source === "prop-value" || source === "async-value" || source === "async-value-member" || source === "async-value-dstr";

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
    // `module-hook-direct` (ADR 0628): the hook returns the creation itself, so it hands back one accessor.
    if (source === "module-hook-direct" && !(sigs.length === 1 && ["async", "memo"].includes(sigs[0].kind))) return false;
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
    case "prop": case "prop-spread": case "prop-value": case "async-prop": case "async-value": case "async-value-member": case "async-loading": return `props.${name}`;
    // ADR 0823: the read is a body destructure of the props object, written where the read is.
    case "async-value-dstr": return `(() => { const { ${name} } = props; return ${name}; })()`;
    case "prop-merge": return `merged.${name}`;
    case "prop-omit": return `rest.${name}`;
    case "context": case "async-context": return `ctx.${name}`;
    case "module-hook-member": return `vals.${name}`;
    case "module-hook-shared": return `shared.${name}`;
    default: return name;
  }
}
export const sourcedRead = (source) => (valueSource(source) ? "value" : "call");

/**
 * The event axis (ADRs 0628 ff., measured in Chrome): rc.21's strict window is event-tied. A body's `untrack(fn, label)` window takes any
 * event, so a listener dispatched synchronously from a body reads in it; an effect's window records its queue's event, so a handler
 * dispatched from an apply, like one dispatched from another handler, reads in none, and so does what a handler or an apply schedules.
 */
/**
 * The fire axis (ADR 0642, measured in Chrome): what delivers a listener on its caller's stack besides `click` and `dispatchEvent`. `steal`
 * moves focus to another element, so the handler at risk is on the element that loses it; `-wrapper` and `-member` are the same call through
 * a helper and a member. A project method or a local function called `focus` delivers nothing (`project`, `shadow`).
 */
/** A fire cell is about the call that delivers: one shape of the read or write inside it, not the whole cross of what the grids vary. */
export const fireShape = (context) => (shape) => !(context.includes("-fire-") || /^(lst|rnd|zero|prop-unread|dny)-|^evt-apply-owned-flush$/.test(context) || ANCHOR_MISSES.includes(context)) || (!shape.mount && (!shape.source || (context === "dny-renamed-prop" && shape.source === "prop")) && !shape.callback && shape.layer === "none" && ["direct", undefined].includes(shape.binding));
const FIRES = ["focus", "select", "blur", "remove", "steal", "reset", "submit", "validity", "modal", "popover", "focus-wrapper", "focus-member"];
/**
 * ADR 0645: what a listener reaches, on the stack of what delivers it. A window listener that calls a member (`method`), a bound member, an
 * object with `handleEvent`, or a function that is also a JSX handler (`shared`). `body` dispatches in the component's body and warns;
 * `none` dispatches nothing and `handler` from a click, and both are silent.
 */
const LISTENER_FORMS = ["method", "module", "bind", "object", "shared"];
const listenersAt = (...positions) => positions.flatMap((position) => LISTENER_FORMS.map((form) => `lst-${position}-${form}`));
/** ADR 0644: `render` drains on its caller's stack: an apply queued before it runs under the body's owner. */
const RENDER_DRAINS = ["rnd-body", "rnd-wrapper", "rnd-root", "rnd-memo", "rnd-foreach"];
const RENDER_LEGAL = ["rnd-handler", "rnd-timer", "rnd-await", "rnd-shadow"];
/** ADR 0645: what `program_listens_for_click` missed, and the elements ADR 0640's exclusion took: each `flush()`es on the apply's stack. */
const ANCHOR_MISSES = ["evt-anchor-apply-bare", "evt-anchor-apply-alias", "evt-anchor-apply-computed", "evt-created-label", "evt-created-checkbox"];
/**
 * The denied events (ADR 0642, the ruling's section 3): delivered by a state change, on the stack of whatever made it, with no dedicated
 * method. Each `dny-*` runs a user's step on an empty stack (`prep`: a real focus, text typed with `execCommand`, a click that opens a
 * popover), then mounts a later root whose body makes the change (`trigger`). An IME composition needs CDP input, which the page cannot
 * make: `compositionend` stays in fixtures.
 */
const DENIED = {
  "dny-blur-disabled": { tag: "button", on: "onBlur", prep: ["EL.focus()"], trigger: "EL.disabled = true" },
  "dny-blur-replace": { tag: "input", on: "onBlur", prep: ["EL.focus()"], trigger: 'EL.replaceWith(document.createElement("span"))' },
  "dny-blur-move": { tag: "input", on: "onBlur", prep: ["EL.focus()"], trigger: "document.body.append(EL)" },
  "dny-focusout-replace": { tag: "input", on: "onFocusOut", prep: ["EL.focus()"], trigger: 'EL.replaceWith(document.createElement("span"))' },
  "dny-change-typed": { tag: "input", on: "onChange", prep: ["EL.focus()", 'document.execCommand("insertText", false, "x")'], trigger: "EL.disabled = true" },
  "dny-change-typed-move": { tag: "input", on: "onChange", prep: ["EL.focus()", 'document.execCommand("insertText", false, "x")'], trigger: "document.body.append(EL)" },
  "dny-beforetoggle-click": { tag: "div", on: "onBeforeToggle", attrs: 'popover="auto"', prep: ["EL.showPopover()"], trigger: 'EL.removeAttribute("popover")' },
  "dny-beforetoggle-write": { tag: "div", on: "onBeforeToggle", attrs: 'popover="auto"', prep: ["EL.showPopover()"], trigger: "EL.popover = null" },
  "dny-formdata": { tag: "form", on: "onFormData", prep: [], trigger: "new FormData(EL)" },
  "dny-formdata-submit": { tag: "form", on: "onFormData", attrs: 'action="javascript:void 0"', prep: [], trigger: "EL.submit()" },
  "dny-focus-selection": { tag: "div", on: "onFocus", attrs: "contenteditable", prep: [], trigger: "getSelection()!.collapse(EL, 0)" },
  "dny-blur-select-remove": { tag: "select", on: "onBlur", prep: ["EL.focus()"], trigger: "(EL as HTMLSelectElement).remove()" },
  // Events of the measured list, so listed calls: `execCommand` needs a focused editable, `ElementInternals` a form-associated element.
  // ADR 0642: `click` and `dispatchEvent` on a receiver no declaration names: the runtime receiver may be an element (`d08`-`d12`).
  "dny-click-any": { tag: "button", on: "onClick", prep: [], trigger: "EL.click()" },
  "dny-dispatch-any": { tag: "button", on: "onClick", prep: [], trigger: 'EL.dispatchEvent(new MouseEvent("click", { bubbles: true }))' },
  "dny-click-structural": { tag: "button", on: "onClick", prep: [], trigger: "const x: { click(): void } = EL; x.click()" },
  "dny-click-interface": { tag: "button", on: "onClick", prep: [], trigger: "interface Clickable { click(): void } const x: Clickable = EL; x.click()" },
  "dny-click-param": { tag: "button", on: "onClick", prep: [], trigger: "((x: any) => x.click())(EL)" },
  "dny-input-exec": { tag: "input", on: "onInput", prep: ["EL.focus()"], trigger: 'document.execCommand("insertText", false, "x")' },
  "dny-invalid-internals": { tag: "x-sc", on: "onInvalid", declares: true, prep: [], trigger: 'const internals: ElementInternals = EL.internals; internals.setValidity({ valueMissing: true }, "m"); internals.checkValidity()' },
};
/** Solid's own DOM work: the focused element leaves on the stack of a `flush()`, a disposer or `insert`, with no call of the program's naming it. */
const DISPOSALS = { "dny-dispose-body": "body", "dny-dispose-memo": "memo", "dny-dispose-compute": "compute", "dny-dispose-untrack": "untrack" };
/** The same triggers on an event of the measured list: nothing delivers it, and it keeps the proof. */
const DENIED_LEGAL = ["dny-legal-keydown", "dny-legal-click", "dny-legal-pointer", "dny-legal-project-click", "dny-blur-lowercase"];
const DENIED_OTHER = ["dny-blur-spread", "dny-typo-event", "dny-blur-dynamic", "dny-portal-dispose", "dny-load-append", "dny-load-for", "dny-load-show", "dny-unmount-flush-body", "dny-unmount-flush-root", "dny-insert-body", "dny-renamed-prop", "dny-shared-fn", "dny-chain-click"];
const DENIED_CONTEXTS = [...Object.keys(DENIED), ...Object.keys(DISPOSALS), ...DENIED_OTHER];
/** `flush()` in a denied handler, with no dispatch in the program (`a04`, `b03`, `b09`): SC2008. */
const DENIED_FLUSH = ["dny-flush-unmount", "dny-flush-load", "dny-flush-popover"];

const ZERO_READS = ["zero-body-empty", "zero-body-var", "zero-await-empty", "zero-await-var"];
const firesAt = (...positions) => positions.flatMap((position) => FIRES.map((fire) => `evt-${position}-fire-${fire}`));
const FIRE_LEGAL = [...firesAt("apply", "handler"), "evt-body-fire-project", "evt-body-fire-shadow"];
const EVENT_LEGAL = [...DENIED_LEGAL, "evt-apply-dispatch", "evt-handler-dispatch", "evt-created-anchor", "timer-from-handler", "micro-from-handler", "timer-from-apply", "micro-from-apply", ...FIRE_LEGAL, ...listenersAt("none", "handler")];

/** Contexts where nothing owns the code that runs: a handler, a timer, a promise or microtask callback, after an await, runWithOwner(null). */
const UNOWNED = ["handler", "timer", "then", "micro", "async", "owner-null"];
/** Contexts that run under a children-capable owner while the code runs, so a write or an action call is refused. */
const OWNED_PURE = ["body", "memo", "effect-compute", "render-compute", "render-apply", "root", "untrack", "jsx", "show", "for"];
/** The leaf scopes that forbid child owners. */
const LEAF = ["settled", "tracked"];
/** STRICT_READ_UNTRACKED's misuse and legal contexts, where its `latest`/`is-pending` variants go. */
const SRU_PENDING_READS = ["body", "effect-apply", "root", "render-apply", "memo", "effect-compute", "jsx", "untrack"];
/** STORE_WRITE_OUTSIDE_SETTER's base contexts. */
const SW_CONTEXTS = [...OWNED_PURE.filter((c) => !["show", "for"].includes(c)), "effect-apply", "handler", "timer", "then", "async"];

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
// rc.17 (a store write there does, so the cleanup runs; both are legal cells of EFFECT_WINDOW_CONTEXTS), and a component rendered in a
// timer reports nothing from its `onSettled`: not a misuse context. A write in a cleanup's `createRoot` is exempt (the disposal); an
// action call there throws.
const FALSE_CLEAN_CONTEXTS = ["comp-slot", "comp-rendered"];
const ASYNC_CONTROL = ["async-loop", "async-for-await", "async-finally"];
/** A handler's async function with the site past a possible await (ADR 0610). */
const HANDLER_ASYNC = ["handler-async-loop", "handler-async-for-await", "handler-async-finally"];
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
// The saturation transforms (model.mjs): T1/T3 hand-off and member-assign layers (HANDOFF_FORMS) and T2 depth layers at the
// 16-layer climb bound (DEPTH_FORMS), with the fixes they exposed (ADRs 0531, 0532, 0534).
export const RECALL_LAYERS = [...CONTROL_LAYERS, ...FILE_LAYERS, ...HANDOFF_FORMS, ...DEPTH_FORMS, ...CHAIN_FORMS];
/**
 * The pair grids (`--pairs`, matrix.mjs `pairShapes`; Fable's report R7 and R10, Astra's proposal 1): two axes the recall grids cross
 * only with the base grid, crossed with each other as enumerated pairs, on the base contexts:
 *   mount-source      a parent that creates the values and mounts the component later or under a row (`show-later`, `for-item`,
 *                     `for-signal`, `loading-on`, and `dynamic` for a module hook: `dynamic()` passes no props) × a prop, a spread, a
 *                     destructured context, a module hook, an async prop or context: the two confirmed false-positive families of
 *                     ADR 0506 (mounts) and ADRs 0511/0518 (sources) behind it
 *   handoff-file      a hand-off form (HANDOFF_FORMS) inside the module helper's callback in each import form (FILE_LAYERS): one
 *                     composite layer `handoff-X+module-Y`, every base binding
 *   callback-handoff  the callback forms `bundle`, `alias`, `shared` × a hand-off form, on the contexts whose site is a callback
 */
export const PAIR_MOUNTS = ["show-later", "for-item", "for-signal", "loading-on", "dynamic"];
export const PAIR_SOURCES = ["prop", "prop-spread", "context-destructured", "module-hook", "async-prop", "async-context"];
export const PAIR_LAYERS = HANDOFF_FORMS.flatMap((handoff) => FILE_LAYERS.map((file) => `${handoff}+${file}`));
export const PAIR_CALLBACKS = ["bundle", "alias", "shared"];
export const PAIR_SETS = ["mount-source", "handoff-file", "callback-handoff"];
/**
 * The recall bindings. Of an API: destructurings of the namespace other than the base `const { flush: flushD } = S` (nested, with a
 * default, with a rest, `let`, exported, through another `const` holding the namespace), a `const` member alias, and Solid re-exported
 * by a project module (`solid-api.ts`, by name or as a namespace). Of a value (a setter, an accessor, an action, a store): a `let` or
 * `var` alias, a conditional, the result of an identity call, a literal with a spread, a computed key or a getter, a member of a local
 * literal, a destructuring default, a nested pattern, an array slot. Callee forms of either: `.bind(null)`, `.call(null, ...)`, an
 * extracted method, an assignment or `||=` callee.
 */
export const RECALL_API_BINDINGS = ["ns-nested", "ns-default", "ns-rest", "ns-let", "ns-exported", "ns-held", "ns-member", "reexport", "reexport-ns", "reexport-ns-as"];
/**
 * A hook that returns a function with a body, not the source (`{ useIt: () => w1() }`), by slot form and by how the slot is read. `-written`
 * crosses them with an unrelated object's member of the same name written elsewhere in the program, which turns the resolver's name veto on:
 * a project hook's slot then keeps the handed net, or nothing placed it (c1 report, S0).
 */
export const HOOK_SLOT_BINDINGS = ["hook-fn-slot", "hook-fn-slot-written", "hook-method-slot-written", "hook-shorthand-fn-written", "hook-fn-member-written"];
export const RECALL_VALUE_BINDINGS = ["let-alias", "var-alias", "cond-alias", "identity", "spread-literal", "computed-key", "getter", "literal-member", "default-slot", "nested", "array-slot"];
/**
 * Type arguments a `.call` spelling needs to type-check under `strict`: `CallableFunction.call` infers from the last overload
 * (`flush<T>(fn: () => T)`) and loses a generic (`useContext`'s `Context<T>`, invariant). The runtime call is unchanged.
 */
const callTypeArguments = (name, args) => (name === "flush" ? "<null, [], void>" : name === "useContext" ? `<null, [typeof ${args}], unknown>` : "");

export const CALLEE_FORMS = ["bind", "call", "extracted", "assign-callee", "or-assign"];

/** An API call: `name` from `module`; `args` is the argument text; `locals` when the arguments name component-scope variables. */
const api = (name, args = "", { module = "solid-js", locals = false } = {}) => ({ kind: "api", module, name, args, locals });
/** A call of a component-scope function (a setter, an accessor, an action). */
const local = (name, args = "") => ({ kind: "local", name, args });
/** A statement on a component-scope object (a store write): `text(object)`. */
const member = (object, text, imports = [], { importLines, files } = {}) => ({ kind: "member", object, text, imports, importLines, files });

/**
 * Per code: the variants, the contexts and bindings to draw from, the component-level signals the call needs (`setup`), the call
 * itself (`call`), and the spec's note on what it takes to fire.
 *   setup(variant, name) -> signals   (name(prefix) -> a fresh identifier)
 *   call(node, sigs) -> api | local | member
 */
/** Store proxy operations (`STORE_WRITE_OUTSIDE_SETTER` `recallVariants`): the text of each on `object`; `nested` needs the nested store. */
/**
 * V-D1: a write expression with no `[[Set]]` on this run, silent in Chrome, and legal in every context. `array`: the store is `[0]`.
 * What keeps its proof is in STORE_OPERATIONS: `logical-or` writes, and a two-element `items` sorts.
 */
const STORE_NO_OPS = {
  "noop-nullish": { text: (o) => `${o}.v ??= 5` },
  "noop-and": { text: (o) => `${o}.v &&= 5` },
  "noop-assign-null": { text: (o) => `Object.assign(${o}, null)` },
  "noop-assign-empty-var": { text: (o) => `(() => { const patch = {}; Object.assign(${o}, patch); })()` },
  "noop-assign-inherited": { text: (o) => `Object.assign(${o}, Object.create({ v: 2 }))` },
  "noop-sort-one": { array: true, text: (o) => `${o}.sort()` },
  "noop-reverse-one": { array: true, text: (o) => `${o}.reverse()` },
  "noop-fill-range": { array: true, text: (o) => `${o}.fill(0, 5)` },
  "noop-fill-empty-range": { array: true, text: (o) => `${o}.fill(0, 1, 1)` },
  "noop-copy-within": { array: true, text: (o) => `${o}.copyWithin(0, 5)` },
};
const STORE_OPERATIONS = {
  compound: { text: (o) => `${o}.v += 1` },
  "logical-or": { text: (o) => `${o}.v ||= 5` },
  increment: { text: (o) => `${o}.v++` },
  bracket: { text: (o) => `${o}["v"] = 7` },
  "object-assign": { text: (o) => `Object.assign(${o}, { v: 7 })` },
  "reflect-set": { text: (o) => `Reflect.set(${o}, "v", 7)` },
  "define-property": { text: (o) => `Object.defineProperty(${o}, "v", { value: 7 })` },
  "reflect-delete": { text: (o) => `Reflect.deleteProperty(${o}, "v")` },
  // ADR 0638: the method runs with the read proxy as `this`.
  "class-method": { classy: true, text: (o) => `${o}.c.inc()` },
  "destructure-assign": { text: (o) => `({ v: ${o}.v } = { v: 7 })` },
  // ADR 0643: the store as a prototype. Each warns (Chrome, rc.21) and no literal proof may clear it: a `__proto__` key or assignment, the
  // reflective forms, and a method that reparents `this`.
  "proto-literal": { text: (o) => `(({ __proto__: ${o} }) as any).x = 7` },
  "proto-create": { text: (o) => `Object.create(${o}).v = 7` },
  "proto-setprototypeof": { text: (o) => `Object.setPrototypeOf({}, ${o}).x = 7` },
  "proto-assign-onto": { text: (o) => `Object.assign(Object.create(${o}), { v: 7 })` },
  "proto-member": { text: (o) => `(() => { const t: any = { a: 1 }; t.__proto__ = ${o}; t.x = 7; })()` },
  "proto-method-this": { text: (o) => `(() => { const t: any = { install() { Object.setPrototypeOf(this, ${o}); } }; t.install(); t.x = 7; })()` },
  "proto-key-handed": { text: (o) => `(() => { const t: any = { a: 1 }; const put = (k: string, v: unknown) => { t[k] = v; }; put("__proto__", ${o}); t.x = 7; })()` },
  "proto-receiver": { text: (o) => `Reflect.set({}, "v", 7, ${o})` },
  "reflect-define": { text: (o) => `Reflect.defineProperty(${o}, "v", { value: 7 })` },
  "nested-held": { nested: true, text: (o) => `(() => { const held = ${o}.a; held.v = 7; })()` },
  ...Object.fromEntries([["pop", "pop()"], ["shift", "shift()"], ["unshift", "unshift(3)"], ["splice", "splice(0, 1, 3)"], ["sort", "sort((a, b) => b - a)"],
    ["reverse", "reverse()"], ["fill", "fill(7)"], ["copy-within", "copyWithin(0, 1)"]].map(([name, call]) => [`items-${name}`, { list: true, text: (o) => `${o}.items.${call}` }])),
  "items-length": { list: true, text: (o) => `${o}.items.length = 0` },
};

/**
 * The rc.17 constructors and readers no base template emits (Fable's report 2026-11-03, R1), each a recall variant measured on the
 * fast runtime first (API_VARIANTS): the signal kind a variant declares (model.mjs `apiSignal`).
 */
const API_KINDS = { "owned-write": "owned", optimistic: "optimistic", "writable-memo": "writable-memo", "optimistic-store": "optimistic-store", "optimistic-store-draft": "optimistic-store", projection: "projection" };
/**
 * The reads of a whole store (ADRs 0620, 0621), each measured on rc.21 in a strict-read window. `reads`: it warns; `array`, `empty`: the
 * store it needs. The silent forms read no key, or none the store holds yet.
 */
const WHOLE_STORE_READS = {
  "store-json": { reads: true, text: (o) => `JSON.stringify(${o})` },
  "store-reflect": { reads: true, text: (o) => `Reflect.get(${o}, "v")` },
  "store-values": { reads: true, text: (o) => `Object.values(${o})` },
  "store-entries": { reads: true, text: (o) => `Object.entries(${o})` },
  "store-assign": { reads: true, text: (o) => `Object.assign({}, ${o})` },
  "store-template": { reads: true, text: (o) => `\`\${${o}}\`` },
  "store-array-from": { reads: true, array: true, text: (o) => `Array.from(${o})` },
  "store-array-spread": { reads: true, array: true, text: (o) => `[...${o}]` },
  "store-for-of": { reads: true, array: true, text: (o) => `for (const x of ${o}) { void x; }` },
  ...STORE_DESTRUCTURES,
  "store-keys": { text: (o) => `Object.keys(${o})` },
  "store-has": { text: (o) => `"v" in ${o}` },
  "store-has-own": { text: (o) => `Object.hasOwn(${o}, "v")` },
  "store-empty-spread": { empty: true, text: (o) => `({ ...${o} })` },
  "store-empty-values": { empty: true, text: (o) => `Object.values(${o})` },
  // ADR 0643: a Get through a prototype is the store's trap. A computed key and a shorthand define an own property.
  "store-proto-literal": { reads: true, text: (o) => `(({ __proto__: ${o} }) as any).v` },
  "store-proto-string": { reads: true, text: (o) => `(({ "__proto__": ${o} }) as any).v` },
  "store-proto-merge": { reads: true, imports: ["merge"], text: (o) => `merge(({ __proto__: ${o} }) as any, { w: 2 }).v` },
  "store-proto-create": { reads: true, text: (o) => `Object.create(${o}).v` },
  "store-proto-setprototypeof": { reads: true, text: (o) => `Object.setPrototypeOf({}, ${o}).v` },
  "store-proto-member": { reads: true, text: (o) => `(() => { const t: any = { a: 1 }; t.__proto__ = ${o}; return t.v; })()` },
  "store-proto-computed": { text: (o) => `(({ ["__proto__"]: ${o} }) as any).v` },
  "store-proto-shorthand": { text: (o) => `((__proto__: unknown) => ({ __proto__ } as any).v)(${o})` },
};
const WHOLE_STORE_WARNING = Object.keys(WHOLE_STORE_READS).filter((form) => WHOLE_STORE_READS[form].reads);
const WHOLE_STORE_SILENT = Object.keys(WHOLE_STORE_READS).filter((form) => !WHOLE_STORE_READS[form].reads);
const STRICT_WINDOWS = ["body", "effect-apply", "root", "render-apply"];
/** ADR 0643: an accessor pair installed on a local, then a plain Get or write: the rules see a member of a plain object, Chrome the signal's. */
const INSTALLED_GETS = {
  "installed-get": (read) => `Object.defineProperty({}, "v", { get: ${read} }).v`,
  "installed-get-create": (read) => `Object.create({}, { v: { get: ${read} } }).v`,
};
const INSTALLED_SETS = {
  "installed-set": (write) => `Object.defineProperty({}, "a", { set: ${write} }).a = 7`,
  "installed-set-many": (write) => `Object.defineProperties({}, { a: { set: ${write} } }).a = 7`,
  "installed-set-reflect": (write) => `(() => { const t: any = {}; Reflect.defineProperty(t, "a", { set: ${write} }); t.a = 7; })()`,
  "installed-set-alias": (write) => `(() => { const t: any = {}; const dp = Object.defineProperty; dp(t, "a", { set: ${write} }); t.a = 7; })()`,
  "installed-set-create": (write) => `Object.create({}, { a: { set: ${write} } }).a = 7`,
};

/** A pending read in another reader: `latest(() => a1())`, `isPending(() => a1())`, or a plain read of a `createDeferred` source. */
const pendingRead = (node, [sig]) => (node.variant === "latest" ? api("latest", `() => ${sig.name}()`, { locals: true }) : node.variant === "is-pending" ? api("isPending", `() => ${sig.name}()`, { locals: true }) : local(sig.name));
/** The owner-attaching constructors a leaf or an effect apply refuses, besides memo, root and effect: a writable memo, a projection. */
const API_PRIMITIVES = { "signal-fn": api("createSignal", "() => 1"), projection: api("createProjection", "(draft: { v: number }) => { draft.v = 1; }, { v: 0 }"), render: api("render", '() => null, document.createElement("div")', { module: "@solidjs/web" }) };

export const SPEC = {
  // Past an await (rc.17 fast runtime, 2026-11-03): no write reports in an async function the body calls, its loop, `for await` or
  // `finally`, an async memo or effect compute, or a handler's async function (ADR 0610).
  REACTIVE_WRITE_IN_OWNED_SCOPE: { legal: ["handler", "timer", "then", "micro", "effect-apply", "async"], recallContexts: [...OWNER_LIFETIME, ...FALSE_CLEAN_CONTEXTS, "memo-shared", "evt-body-dispatch", "evt-apply-owned-flush", ...DENIED_CONTEXTS, ...firesAt("body", "memo", "compute", "untrack"), ...listenersAt("body"), ...RENDER_DRAINS],
    recallLegal: [...EVENT_LEGAL, ...RENDER_LEGAL, ...ASYNC_CONTROL, "memo-async", "effect-async", "memo-async-loop", "memo-async-for-await", "memo-async-finally", "effect-async-loop", "effect-async-for-await", "effect-async-finally", ...HANDLER_ASYNC],
    variants: ["signal", "store", "refresh"], contexts: OWNED_PURE, bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => [variant === "store" ? storeSignal(name("w"), 0) : variant === "refresh" ? memoSignal(name("m")) : API_KINDS[variant] ? apiSignal(name("w"), 0, API_KINDS[variant]) : signal(name("w"), 0, "probe")],
    call: (node, [sig]) => (INSTALLED_SETS[node.variant] ? member(sig.setter, INSTALLED_SETS[node.variant]) : node.variant === "store" ? local(sig.setter, "(s) => { s.v = 7; }") : node.variant === "refresh" ? api("refresh", sig.name, { locals: true }) : local(sig.setter, "7")),
    // refresh is an API; the other two are local functions, which have no namespace form.
    bindingsFor: (variant) => (variant === "refresh" ? BINDINGS : INSTALLED_SETS[variant] ? ["direct", "alias", "wrapper"] : ["direct", "alias", "destructured", "wrapper"]),
    // API_VARIANTS (recall; measured on rc.17's fast runtime, 2026-11-03): a writable memo's setter (`createSignal(() => 0)`) reports in
    // every owned context and is legal in a handler and an apply; an optimistic setter (`createOptimistic(0)`) and a signal created
    // `{ ownedWrite: true }` report nothing anywhere, so both are legal in the owned contexts too (`legalVariantContexts`).
    recallVariants: ["writable-memo", ...Object.keys(INSTALLED_SETS)], recallLegalVariants: ["writable-memo", "optimistic", "owned-write"],
    variantContexts: { "writable-memo": [...OWNED_PURE, "handler", "effect-apply"], optimistic: [...OWNED_PURE, "handler"], "owned-write": [...OWNED_PURE, "handler"] },
    legalVariantContexts: { optimistic: OWNED_PURE, "owned-write": OWNED_PURE },
  },
  STORE_WRITE_OUTSIDE_SETTER: { legal: ["handler", "timer", "then"], recallContexts: [...OWNER_LIFETIME, "effect-cleanup-rerun", ...ASYNC_CONTROL],
    variants: ["assign", "delete"],
    contexts: SW_CONTEXTS,
    bindings: ["direct", "alias", "wrapper"],
    // Grid-only (`gridVariantsOf`): writes past the root (a nested record's field, an array mutator), and a write deferred out of the
    // store's own setter callback (a microtask, a timer, a promise callback), which runs once the draft has closed and is dropped
    // (ADR 0476; Chrome-witnessed in probes/store-setter). A deferred write calls the setter, which is itself a defect in an owned
    // scope, so it runs only where the call is legal (`variantContexts`).
    gridVariants: ["nested-assign", "nested-push", "deferred-micro", "deferred-timer", "deferred-then"],
    variantContexts: { "deferred-micro": ["handler", "timer", "then"], "deferred-timer": ["handler", "timer", "then"], "deferred-then": ["handler", "timer", "then"],
      "optional-link": ["body", "memo", "effect-apply", "handler"], "optional-link-leaf": ["body", "handler"], "optional-link-draft": ["handler"], "async-setter": ["handler"],
      "optimistic-store": SW_CONTEXTS, projection: SW_CONTEXTS, "optimistic-store-draft": ["handler", "timer", "then"] },
    // Legal (legal.mjs): a write through the draft, and one through the read proxy inside its own setter, synchronously; both commit.
    legalVariants: ["draft", "in-setter"],
    // Recall only (`recallVariants`, catalogue rank 6): the other ways code mutates a store proxy, each measured on rc.17 (2026-11-01,
    // 700 runs): every one warns this code outside the store's setter (a body, a handler), and none commits. Under the setter they
    // are legal, as is a write to a copy (`recallLegalVariants`).
    // Typed away on purpose (TYPED_VARIANTS, tsc-overlap.mjs): a write through an optional link (`s1.a.v = 7`, TS18048; `-leaf`
    // is open-work.md's `a?: { b?: number }` form, which it records SC2003 still reports), the same through the draft in its
    // setter, and an async setter callback (TS2345,
    // ASYNC_STORE_SETTER's shape) whose write lands after an `await`, outside the draft.
    // API_VARIANTS (measured 2026-11-03): a write through an optimistic store's or a projection's read proxy reports in every context;
    // an optimistic store's draft write is legal.
    recallVariants: [...Object.keys(STORE_OPERATIONS), "optional-link", "optional-link-leaf", "async-setter", "optimistic-store", "projection"],
    // ADR 0626: a shallow store serves values raw, so a write past its root lands and warns nothing; a root write is dropped as ever.
    recallLegalVariants: ["copy-spread", "copy-assign", "slice-push", "draft-push", "draft-compound", "optional-link-draft", "optimistic-store-draft", "shallow-nested", "shallow-items", ...Object.keys(STORE_NO_OPS)],
    setup: (variant, name) => [STORE_NO_OPS[variant]?.array ? storeArraySignal(name("s"), 0) : variant === "class-method" ? storeClassSignal(name("s"), 0) : variant?.startsWith("shallow-") ? storeShallowSignal(name("s"), 0) : API_KINDS[variant] ? apiSignal(name("s"), 0, API_KINDS[variant]) : variant?.startsWith("optional-link") ? storeLinkSignal(name("s"), 0, variant === "optional-link-leaf") : variant === "delete" ? storeOptSignal(name("s"), 0) : STORE_OPERATIONS[variant]?.list || ["slice-push", "draft-push"].includes(variant) ? storeListSignal(name("s"), 0) : variant?.startsWith("nested") || STORE_OPERATIONS[variant]?.nested ? storeNestedSignal(name("s"), 0) : storeSignal(name("s"), 0)],
    call: (node, [sig]) => member(sig.name, (object) => {
      const deferred = { "deferred-micro": (w) => `queueMicrotask(() => { ${w}; })`, "deferred-timer": (w) => `setTimeout(() => { ${w}; }, 0)`, "deferred-then": (w) => `Promise.resolve().then(() => { ${w}; })` }[node.variant];
      if (deferred) return `${sig.setter}(() => { ${deferred(`${object}.v = 7`)}; })`;
      if (node.variant === "in-setter") return `${sig.setter}(() => { ${object}.v = 7; })`;
      if (node.variant === "draft") return `${sig.setter}((draft) => { draft.v = 7; })`;
      if (STORE_OPERATIONS[node.variant]) return STORE_OPERATIONS[node.variant].text(object);
      if (STORE_NO_OPS[node.variant]) return STORE_NO_OPS[node.variant].text(object);
      if (node.variant === "shallow-nested") return `${object}.a.v = 7`;
      if (node.variant === "shallow-items") return `${object}.items.push(7)`;
      if (node.variant === "copy-spread") return `({ ...${object} }).v = 7`;
      if (node.variant === "copy-assign") return `Object.assign({}, ${object}, { v: 7 })`;
      if (node.variant === "slice-push") return `${object}.items.slice().push(7)`;
      if (node.variant === "draft-push") return `${sig.setter}((draft) => { draft.items.push(7); })`;
      if (node.variant === "draft-compound") return `${sig.setter}((draft) => { draft.v += 1; })`;
      if (node.variant === "optimistic-store-draft") return `${sig.setter}((draft) => { draft.v = 7; })`;
      if (node.variant === "optional-link") return `${object}.a.v = 7`;
      if (node.variant === "optional-link-leaf") return `${object}.a.b = 1`;
      if (node.variant === "optional-link-draft") return `${sig.setter}((draft) => { draft.a.v = 7; })`;
      if (node.variant === "async-setter") return `${sig.setter}(async (draft) => { await Promise.resolve(); draft.v = 7; })`;
      return node.variant === "delete" ? `delete ${object}.v` : node.variant === "nested-assign" ? `${object}.a.v = 7` : node.variant === "nested-push" ? `${object}.items.push(1)` : `${object}.v = 7`;
    }),
  },
  // `settled`: an async memo read after it settled (ADR 0511 review: probe pending-async/hook-shared-instance, rc.17 warns this code in an
  // apply, no pending throw), through the module-scope hook result beside the body's own call.
  STRICT_READ_UNTRACKED: { legal: ["memo", "effect-compute", "render-compute", "jsx", "untrack"], recallContexts: ["evt-body-dispatch", ...firesAt("body"), ...listenersAt("body"), ...DENIED_CONTEXTS.filter((context) => !context.startsWith("dny-unmount-flush") && !["dny-dispose-memo", "dny-dispose-compute", "dny-dispose-untrack"].includes(context))], recallLegal: [...EVENT_LEGAL, ...ZERO_READS, "handed-flush"],
    // `settled` is a grid variant (never drawn at random, so random programs and combinations' extras keep theirs) and a legal one.
    variants: ["signal", "memo"], gridVariants: ["settled"], legalVariants: ["signal", "memo", "settled"],
    contexts: ["body", "effect-apply", "root", "render-apply"], bindings: ["direct", "alias", "destructured", "wrapper"],
    // Typed away on purpose (TYPED_VARIANTS): an accessor used uncalled where TypeScript rejects it (uncalled-accessor's excluded
    // positions: arithmetic, TS2362; an index, TS2538), misuse and legal alike; any violation there is tsc-overlap.mjs's.
    // API_VARIANTS (measured 2026-11-03): `latest(() => a1())` and `isPending(() => a1())` of a pending async memo are strict reads
    // (not a pending throw): reported where a read is, silent in a memo, a compute, JSX, `untrack` and a leaf (`legalVariantContexts`).
    // `store-link` (ADR 0615): a store's Get, the read form a call of an accessor and a member read reach by different paths.
    // ADRs 0620, 0621: the reads of a whole store. A warning form is legal where a read is, a silent one in every window too.
    // ADR 0629: `untrack` scopes the wrapped read only, so a Get of the proxy it returns warns (`store-escaped`), and a primitive it
    // returns reads nothing (`store-escaped-leaf`). `labelled` (ADR 0628): `untrack(fn, "label")` opens a window wherever it runs.
    // Rules audit (reproduced in Chrome): a `merge` key a literal serves reads nothing, and a raw store root is no proxy (ADRs 0630, 0634);
    // `latest(a)`, `isPending(a)` by name and over a pending store warn (ADR 0633); a declared source's body read warns (ADR 0632), and
    // a compute that answers synchronously does too, with no pending throw (ADR 0631).
    recallVariants: ["uncalled-arith", "uncalled-index", "latest", "is-pending", "store-link", "store-deep", "store-escaped", "labelled", "latest-name", "is-pending-name", "latest-store", "is-pending-store", "declared", "maybe-sync", ...Object.keys(INSTALLED_GETS), ...WHOLE_STORE_WARNING],
    recallLegalVariants: ["uncalled-arith", "latest", "is-pending", "store-link", "store-deep", "store-escaped", "store-escaped-leaf", "store-snapshot", "merge-plain", "store-raw-root", "declared", ...WHOLE_STORE_WARNING, ...WHOLE_STORE_SILENT],
    bindingsFor: (variant) => (variant?.startsWith("uncalled") || variant?.startsWith("store-") || INSTALLED_GETS[variant] ? ["direct", "alias", "wrapper"] : ["direct", "alias", "destructured", "wrapper"]),
    variantContexts: { settled: ["body", "effect-apply", "root", "render-apply"], "uncalled-arith": ["body", "effect-apply", "memo"], "uncalled-index": ["body", "effect-apply"],
      latest: [...SRU_PENDING_READS, "settled", "tracked"], "is-pending": [...SRU_PENDING_READS, "settled", "tracked"], labelled: [...STRICT_WINDOWS, "memo", "effect-compute", "render-compute", "handler"],
      "latest-name": STRICT_WINDOWS, "is-pending-name": STRICT_WINDOWS, "latest-store": ["body"], "is-pending-store": ["body"], declared: ["body", "untrack"], "maybe-sync": ["body"] },
    legalVariantContexts: { latest: LEAF, "is-pending": LEAF, "store-snapshot": STRICT_WINDOWS, "store-escaped-leaf": STRICT_WINDOWS, "merge-plain": STRICT_WINDOWS, "store-raw-root": STRICT_WINDOWS, declared: ["untrack"], ...Object.fromEntries(WHOLE_STORE_SILENT.map((form) => [form, STRICT_WINDOWS])) },
    // A pending read in JSX or a memo the JSX reads is under `<Loading>` (`legalLoading`, see PENDING_ASYNC_UNTRACKED_READ); in a render
    // effect's compute no JSX boundary covers it, so `latest`/`is-pending` take no `render-compute` cell (SRU_PENDING_READS).
    legalLoading: { memo: ["latest", "is-pending"], jsx: ["latest", "is-pending"] },
    variantSources: { settled: ["module-hook-shared"] },
    setup: (variant, name) => [variant === "memo" ? memoSignal(name("m")) : ["settled", "latest", "is-pending"].includes(variant) ? asyncSignal(name("a")) : variant === "store-link" || variant?.startsWith("store-escaped") ? storeNestedSignal(name("s"), 0) : variant === "store-deep" || variant === "store-snapshot" || variant === "merge-plain" ? storeSignal(name("s"), 0) : variant === "store-raw-root" ? storeRawSignal(name("s"), 0) : variant === "latest-store" || variant === "is-pending-store" ? storeAsyncSignal(name("s"), 0) : variant === "declared" ? asyncDeclaredSignal(name("a")) : variant === "maybe-sync" ? asyncMaybeSignal(name("a")) : WHOLE_STORE_READS[variant] ? (WHOLE_STORE_READS[variant].array ? storeArraySignal : WHOLE_STORE_READS[variant].empty ? storeEmptySignal : storeSignal)(name("s"), 0) : signal(name("r"), 3, "src")],
    call: (node, sigs) => (WHOLE_STORE_READS[node.variant] ? member(sigs[0].name, WHOLE_STORE_READS[node.variant].text, WHOLE_STORE_READS[node.variant].imports) : INSTALLED_GETS[node.variant] ? member(sigs[0].name, INSTALLED_GETS[node.variant]) : node.variant === "merge-plain" ? member(sigs[0].name, (o) => `merge(${o}, { b: 2 }).b`, ["merge"]) : node.variant === "store-raw-root" ? member(sigs[0].name, (o) => `${o}.v`) : node.variant === "latest-name" ? api("latest", sigs[0].name, { locals: true }) : node.variant === "is-pending-name" ? api("isPending", sigs[0].name, { locals: true }) : node.variant === "latest-store" ? api("latest", `() => ${sigs[0].name}.v`, { locals: true }) : node.variant === "is-pending-store" ? api("isPending", `() => ${sigs[0].name}.v`, { locals: true }) : node.variant === "store-escaped" ? member(sigs[0].name, (o) => `untrack(() => ${o}.a).v`, ["untrack"]) : node.variant === "store-escaped-leaf" ? member(sigs[0].name, (o) => `untrack(() => ${o}.v)`, ["untrack"]) : node.variant === "labelled" ? member(sigs[0].name, (o) => `untrack(() => ${o}(), "scope")`, ["untrack"]) : node.variant === "store-deep" ? api("deep", sigs[0].name, { locals: true }) : node.variant === "store-snapshot" ? api("snapshot", sigs[0].name, { locals: true }) : node.variant === "uncalled-arith" ? member(sigs[0].name, (o) => `${o} - 1`) : node.variant === "uncalled-index" ? member(sigs[0].name, (o) => `[1, 2][${o}]`) : node.variant === "store-link" ? member(sigs[0].name, (o) => `${o}.a`) : pendingRead(node, sigs)),
  },
  NO_OWNER_CLEANUP: { legal: ["body", "memo", "effect-compute", "render-compute", "root"], recallLegal: [...RENDER_DRAINS, "zero-await-empty", "zero-await-var"], recallContexts: ASYNC_CONTROL, contexts: UNOWNED, bindings: BINDINGS, setup: () => [], call: () => api("onCleanup", "() => {}") },
  // `reaction-track` (grid only): `track1(() => 1)` of a tracker `helpers.ts` creates at module scope. rc.17's tracker creates its
  // effect under the owner current at `createReaction` (`@solidjs/signals` dist/dev.js:2630-2651, `runWithOwner(owner, …)`): none
  // there, so every `track1()` warns, wherever it is called. `reaction-track-owned` (legal only): the tracker the component (or its
  // parent, or a hook the body calls) creates, owned there, so a call anywhere is legal.
  NO_OWNER_EFFECT: { legal: ["body", "root"], recallContexts: ASYNC_CONTROL,
    variants: ["effect", "render"], gridVariants: ["reaction-track"], recallVariants: ["reaction-track-hook"], legalVariants: ["effect", "render", "reaction-track-owned"],
    variantContexts: { "reaction-track-hook": UNOWNED },
    variantSources: { "reaction-track": ["module", "module-barrel"],
      "reaction-track-owned": ["local", ...SOURCE_AXES.props.filter((source) => source !== "prop-value"), ...SOURCE_AXES.context, "module-hook", "module-hook-member"] },
    contexts: UNOWNED, bindings: BINDINGS, bindingsFor: (variant) => (variant === "reaction-track-hook" ? ["direct"] : variant?.startsWith("reaction-track") ? ["direct", "alias", "destructured", "wrapper"] : BINDINGS),
    setup: (variant, name) => (variant !== "reaction-track-hook" && variant?.startsWith("reaction-track") ? [reactionSignal(name("track"))] : []),
    // ADR 0551: a tracker a hook makes, in a call with no owner.
    call: (node, sigs) => (node.variant === "reaction-track-hook" ? member("", () => "(() => { const useMade = () => createReaction(() => {}); useMade()(() => 1); })()", ["createReaction"]) : node.variant?.startsWith("reaction-track") ? local(sigs[0].name, "() => 1")
      : api(node.variant === "render" ? "createRenderEffect" : "createEffect", "() => 1, () => {}")),
  },
  NO_OWNER_BOUNDARY: { recallContexts: ASYNC_CONTROL,
    variants: ["loading", "errored"], contexts: UNOWNED, bindings: BINDINGS, setup: () => [],
    call: (node) => api(node.variant === "errored" ? "Errored" : "Loading", "{ fallback: 0, children: 1 }"),
  },
  NO_OWNER_CONTEXT: { legal: ["body", "memo", "root"], recallContexts: ASYNC_CONTROL, contexts: UNOWNED, bindings: BINDINGS, setup: () => [], contextLine: "const Ctx = createContext(7);", call: () => api("useContext", "Ctx") },
  CONTEXT_NOT_FOUND: { recallVariants: ["default-undefined"], variantContexts: { "default-undefined": ["body", "memo", "render-compute", "root", "jsx", "show", "for"] },
    contexts: ["body", "memo", "render-compute", "root", "jsx", "show", "for"], bindings: BINDINGS, setup: () => [],
    // `default-undefined` (recall): `createContext<number | undefined>(undefined)` is default-less too (CHEATSHEET.md; rc.17
    // `getContext`, packages/signals/src/core/context.ts:38-52: an `undefined` value falls to the default, `undefined` again throws).
    contextLine: (variant) => (variant === "default-undefined" ? "const Ctx = createContext<number | undefined>(undefined);" : "const Ctx = createContext<number>();"),
    call: () => api("useContext", "Ctx"),
  },
  SETTLED_CLEANUP_UNOWNED: { legal: ["body", "root"], recallContexts: ASYNC_CONTROL, contexts: [...UNOWNED, "effect-apply"], bindings: BINDINGS, setup: () => [], call: () => api("onSettled", "() => () => {}") },
  CLEANUP_IN_FORBIDDEN_SCOPE: { legal: ["body", "root"], recallContexts: ["tracked-rerun"], contexts: LEAF, bindings: BINDINGS, setup: () => [], call: () => api("onCleanup", "() => {}") },
  // `signal-fn`, `projection` (API_VARIANTS, measured 2026-11-03): a writable memo and a projection attach an owner like a memo:
  // reported in a leaf and an effect apply, legal in a body and a root.
  PRIMITIVE_IN_FORBIDDEN_SCOPE: { legal: ["body", "root"], recallContexts: ["tracked-rerun"],
    recallVariants: ["signal-fn", "projection", "render"], recallLegalVariants: ["signal-fn", "projection"], variantContexts: { "signal-fn": [...LEAF, "body", "root"], projection: [...LEAF, "body", "root"], render: LEAF },
    variants: ["memo", "root", "effect"], contexts: LEAF, bindings: BINDINGS, setup: () => [],
    call: (node) => API_PRIMITIVES[node.variant] ?? (node.variant === "root" ? api("createRoot", "() => 1") : node.variant === "effect" ? api("createEffect", "() => 1, () => {}") : api("createMemo", "() => 1")),
  },
  ASYNC_OUTSIDE_LOADING_BOUNDARY: { recallOnly: true, recallContexts: ["jsx", "show", "for"],
    contexts: [], bindings: ["direct", "alias", "destructured", "wrapper"], placeless: true,
    setup: (variant, name) => [asyncSignal(name("a"))], call: (node, [sig]) => local(sig.name),
  },
  // rc.17 dev-shared.js: a pending read inside createTrackedEffect or onSettled serves the committed value (and throws while uninitialized).
  // `deferred` (API_VARIANTS, measured 2026-11-03): a `createDeferred(async () => 1)` source read while pending reports both codes
  // where an async memo does.
  PENDING_ASYNC_FORBIDDEN_SCOPE: { recallLegal: ["zero-settled-empty", "zero-settled-var"], recallContexts: ["tracked-rerun", ...firesAt("settled")], recallVariants: ["deferred"], variantContexts: { deferred: LEAF },
    contexts: LEAF, bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => [variant === "deferred" ? apiSignal(name("a"), 0, "deferred", "src") : asyncSignal(name("a"))], call: (node, sigs) => pendingRead(node, sigs),
  },
  // The `-guarded` applies (ADR 0504 review): the read sits behind a guard that returns on the first run, which is the only run;
  // `-closure`: the guard is in a closure the apply calls. `-rerun`: the compute reads a signal a button sets to 2, so the apply runs again
  // and the guarded read executes (the source settled by then): Chrome then judges the strict read the checker reports there.
  // A module-scope async memo is created with no owner and is still pending when a `-guarded-rerun` apply runs again (Chrome reports
  // the read there): those two legal contexts do not take the `module` source, nor the recall source that re-exports it through a
  // barrel (`module-barrel`; 84 legal programs Chrome reported, 2026-10-30).
  // `legalLoading`: a legal program whose JSX reads the pending value (the `memo` cell's `{m1()}`) sits under `<Loading>`: without
  // one rc.17 reports ASYNC_OUTSIDE_LOADING_BOUNDARY, advisory but placeless, which made the legal judge call a program with a violation
  // at its site `disturbed` instead of judging it (60 lg-recall programs on the 644bd1a81 run, the `cb-*` forms of `memo`).
  // `untrack` (ADRs 0619, 0628): rc.21 throws for a source the body running it created, directly or through a hook it calls. A module
  // source or a parent's is another owner's: the read suspends, so no code (and a body re-run onto a fresh memo never ends).
  // `labelled`: `untrack(fn, "label")` opens a window of its own, so it throws on any pending source.
  PENDING_ASYNC_UNTRACKED_READ: { recallLegal: ["prop-unread-cap", "prop-unread-member", "prop-unread-lower"], legalLoading: { memo: true }, sourcesNotIn: { "effect-apply-guarded-rerun": ["module", "module-barrel"], "render-apply-guarded-rerun": ["module", "module-barrel"], untrack: ["module", "module-barrel", "async-prop", "async-value", "async-value-member", "async-context", "async-loading"] }, legal: ["memo", "effect-apply-guarded", "render-apply-guarded", "effect-apply-guarded-closure", "render-apply-guarded-closure", "effect-apply-guarded-rerun", "render-apply-guarded-rerun"],
    // ADR 0635: an uninitialized derive refuses key probes.
    recallVariants: ["deferred", "labelled", "store-keys", "store-in", "store-has-own", "over-async"], recallMountsNotIn: { untrack: ["dynamic"] }, bindingsFor: (variant) => (variant?.startsWith("store-") ? ["direct", "alias", "wrapper"] : ["direct", "alias", "destructured", "wrapper"]),
    variantContexts: { deferred: ["body", "root", "effect-apply", "render-apply"], labelled: ["body", "root"], "store-keys": ["body", "root"], "store-in": ["body", "root"], "store-has-own": ["body", "root"], "over-async": ["body", "root", "untrack"] },
    contexts: ["body", "root", "effect-apply", "render-apply", "untrack"], bindings: ["direct", "alias", "destructured", "wrapper"],
    // `over-async`: a sync memo over an async one is pending with it (ADR 0628).
    setup: (variant, name) => [variant === "deferred" ? apiSignal(name("a"), 0, "deferred", "src") : variant === "over-async" ? apiSignal(name("a"), 0, "over-async", "src") : variant?.startsWith("store-") ? storeAsyncSignal(name("s"), 0) : asyncSignal(name("a"))],
    call: (node, sigs) => (node.variant === "store-keys" ? member(sigs[0].name, (o) => `Object.keys(${o})`) : node.variant === "store-in" ? member(sigs[0].name, (o) => `"v" in ${o}`) : node.variant === "store-has-own" ? member(sigs[0].name, (o) => `Object.hasOwn(${o}, "v")`) : node.variant === "labelled" ? member(sigs[0].name, (o) => `untrack(() => ${o}(), "scope")`, ["untrack"]) : pendingRead(node, sigs)),
  },
  SYNC_NODE_RECEIVED_ASYNC: {
    variants: ["memo", "effect"], recallVariants: Object.keys(SYNC_COMPUTES), contexts: ["body", "root", "untrack", "memo", "jsx"], bindings: BINDINGS, setup: () => [],
    call: (node) => (SYNC_COMPUTES[node.variant] ? api("createMemo", SYNC_COMPUTES[node.variant]) : node.variant === "effect" ? api("createEffect", "async () => 1, () => {}, { sync: true }") : api("createMemo", "async () => 1, { sync: true }")),
  },
  UNTRACKED_READ_AFTER_AWAIT: { recallContexts: ["memo-async-loop", "memo-async-for-await", "memo-async-finally", "effect-async-loop", "effect-async-for-await", "effect-async-finally"],
    contexts: ["memo-async", "effect-async"], bindings: ["direct", "alias", "destructured", "wrapper"],
    recallVariants: ["store-link"], bindingsFor: (variant) => (variant === "store-link" ? ["direct", "alias", "wrapper"] : ["direct", "alias", "destructured", "wrapper"]),
    setup: (variant, name) => [variant === "store-link" ? storeNestedSignal(name("s"), 0) : signal(name("r"), 3, "src")], call: (node, [sig]) => (node.variant === "store-link" ? member(sig.name, (o) => `${o}.a`) : local(sig.name)),
  },
  // Defect codes no rule claims (codes.mjs `defect-unclaimed`): measured for "never quieter than Solid"; a silent checker is a false clean.
  // rc.17 dev-shared.js `assertNotInEffectCallback`: a memo, effect or root created with no owner in an effect callback belongs to nobody.
  PRIMITIVE_IN_EFFECT_CALLBACK: { legal: ["body", "root"], recallContexts: ["effect-apply-rerun", "effect-apply-guard-open"],
    // Effect apply only: in a render effect's apply rc.17 does not report it (the fast path fires it in every effect-apply cell).
    recallVariants: ["signal-fn", "projection", "render"], recallLegalVariants: ["signal-fn", "projection"], variantContexts: { "signal-fn": ["effect-apply", "body", "root"], projection: ["effect-apply", "body", "root"], render: ["effect-apply"] },
    variants: ["memo", "root", "effect"], contexts: ["effect-apply"], bindings: BINDINGS, setup: () => [],
    call: (node) => API_PRIMITIVES[node.variant] ?? (node.variant === "root" ? api("createRoot", "() => 1") : node.variant === "effect" ? api("createEffect", "() => 1, () => {}") : api("createMemo", "() => 1")),
  },
  // rc.17 signals dev.js: a `<Loading on>` re-armed by a write while a source it waits on is also read outside it (the fallback can
  // never show). SC5006 claims it as an obligation only (ADR 0517). The site is the outside read; the diagnostic is raised from the
  // boundary's notification (no authored frame is guaranteed), so it is judged by file. Recall grids only (`recallOnly`).
  // rc.21 (ADR 0625): `onFlush` under a children-capable owner warns. Measured on the dev build: a body, a compute, a root; silent in an
  // apply, a leaf and a handler. Recall grids only.
  ON_FLUSH_IN_OWNED_SCOPE: { recallOnly: true, legal: ["handler", "effect-apply", "settled", "tracked"],
    contexts: [], recallContexts: ["body", "memo", "effect-compute", "render-compute", "root"], bindings: BINDINGS, setup: () => [], call: () => api("onFlush", "() => {}") },
  // rc.21 (ADR 0636): an optimistic setter before its async source's first answer. A driven click lands after it: legal, an obligation.
  // `creates-here`: the creation is on the write's stack, so a handler throws too (ADR 0636); `declared`: a `loadingValue` is an answer.
  OPTIMISTIC_WRITE_BEFORE_LOAD: { recallOnly: true, variants: ["signal", "store"], recallVariants: ["creates-here"], recallLegalVariants: ["declared", "tuple-plain-slot"], recallLegal: ["handler"],
    variantContexts: { signal: ["body", "root"], store: ["body", "root"], "creates-here": ["body", "creates-in-handler"] }, legalVariantContexts: { declared: ["handler"], "tuple-plain-slot": ["handler"] },
    contexts: [], recallContexts: ["body", "root", "creates-in-handler"], bindings: ["direct", "alias", "wrapper"],
    setup: (variant, name) => (variant === "creates-here" ? [] : [apiSignal(name("o"), 1, variant === "store" ? "optimistic-async-store" : variant === "declared" ? "optimistic-declared" : variant === "tuple-plain-slot" ? "optimistic-tuple-hook" : "optimistic-async", "src")]),
    call: (node, sigs) => (node.variant === "creates-here" ? member("createOptimistic", () => `createOptimistic(async () => 1)[1](2)`, ["createOptimistic"]) : node.variant === "store" ? local(sigs[0].setter, "(d) => { d.v = 2; }") : node.variant === "tuple-plain-slot" ? local(sigs[0].name) : local(sigs[0].setter, "2")),
  },
  // rc.21 (ADR 0637): a compute catches a pending read's throw and completes. `rethrow` lets it continue; a signal never throws.
  // Per pass (ADR 0637): `store-get`, `store-keys` and `latest` read too; `prior-read` and `later-read` leave a read outside the `try`,
  // so the throw leaves the pass and nothing reports.
  CAUGHT_REACTIVE_READ: { recallOnly: true, placeless: true, variants: ["async"], recallVariants: ["store-get", "store-keys", "latest", "hook-call", "get-literal"], legalVariants: ["rethrow", "signal", "prior-read", "later-read", "owned-setter"], recallLegal: ["memo", "effect-compute"], legalLoading: { memo: true }, noCatchLayer: true,
    contexts: [], recallContexts: ["memo", "effect-compute"], bindings: ["direct"],
    setup: (variant, name) => [variant === "owned-setter" ? apiSignal(name("w"), 0, "owned") : variant === "signal" ? signal(name("r"), 1, "src") : variant?.startsWith("store-") ? storeAsyncSignal(name("s"), 0) : asyncSignal(name("a"))],
    // ADR 0640: a setter in a `try` is no read: `ownedWrite` is the one compute a setter call is legal in.
    call: (node, [sig]) => (node.variant === "owned-setter" ? member(sig.setter, (o) => `try { ${o}(1); } catch { }`) : node.variant === "latest" ? member(sig.name, (o) => `try { latest(${o}); } catch { }`, ["latest"]) : member(sig.name, (o) => ({
      rethrow: `try { ${o}(); } catch (error) { throw error; }`, "prior-read": `${o}(); try { ${o}(); } catch { }`, "later-read": `try { ${o}(); } catch { } ${o}()`,
      "store-get": `try { ${o}.v; } catch { }`, "store-keys": `try { Object.keys(${o}); } catch { }`,
      // ADR 0658: no accessor call the block shows: a value a hook handed over, and a literal's getter.
      "hook-call": `const useRead = () => ${o}; const read = useRead(); try { read(); } catch { }`,
      "get-literal": `const held = { get v() { return ${o}(); } }; try { held.v; } catch { }` }[node.variant] ?? `try { ${o}(); } catch { }`))),
  },
  // rc.21 (ADR 0624): `<Loading on={fn}>` warns on the boundary's first run. The site is a read under the boundary; the `on` value is
  // the variant. Judged by file (the diagnostic is raised from the boundary). `on` is typed `any`. Recall grids only.
  LOADING_ON_ACCESSOR: { recallOnly: true, placeless: true, variants: ["accessor", "memo", "arrow"], legalVariants: ["value"], legal: ["loading-on-accessor"],
    contexts: [], recallContexts: ["loading-on-accessor"], bindings: ["direct", "alias", "wrapper"],
    setup: (variant, name) => [variant === "memo" ? memoSignal(name("t")) : signal(name("t"), 1, "src")], call: (node, [sig]) => local(sig.name),
    onAttribute: (variant, sig) => (variant === "arrow" ? `() => ${sig}()` : variant === "value" ? `${sig}()` : sig) },
  LOADING_ON_OUTSIDE_HOLD: { recallOnly: true, placeless: true,
    contexts: [], recallContexts: ["loading-on-outside"], bindings: ["direct", "alias", "wrapper"],
    setup: (variant, name) => loadingSignals(name),
    call: (node, [, sig]) => local(sig.name),
  },
  ASYNC_STORE_SETTER: {
    typedAway: true, contexts: [], bindings: [], setup: () => [], call: () => api("createStore"),
    probe: 'import { createStore } from "solid-js";\nexport function C1() {\n  const [s1, setS1] = createStore({ v: 0 });\n  return <button onClick={() => { setS1(async (s) => { await Promise.resolve(); s.v = 7; }); }}>{s1.v}</button>;\n}\n',
    why: "the typings reject it: the setter callback is `(state) => void | T`, so an async callback is TS2345 and tsc already reports it",
  },
  // rc.17 `createEffect` (`@solidjs/signals` dist/dev.js:2504-2518) reports and throws this code when `effectFn === undefined`, before
  // anything else. The one-argument call is TS2554, so the rule's claim (missing-effect-function.md) is what type-checks: an apply a
  // cast hid (`undefined as unknown as () => void`, a violation) or an `any` (uncertifiable). Recall grids only (`recallOnly`), so
  // the random draws and the base grids keep their programs. `one-arg` and `spread-one` (TS2554, TS2556) are typed away on purpose
  // (TYPED_VARIANTS): the rule must stay silent there, and a violation is tsc-overlap.mjs's.
  MISSING_EFFECT_FN: { recallOnly: true,
    contexts: [], recallContexts: ["body", "root", "handler"], variants: ["cast-fn", "cast-any"], recallVariants: ["one-arg", "spread-one"],
    bindings: BINDINGS, setup: () => [],
    call: (node) => api("createEffect", { "cast-any": "() => 1, undefined as any", "one-arg": "() => 1", "spread-one": "...([() => 1] as const)" }[node.variant] ?? "() => 1, undefined as unknown as () => void"),
  },
  ACTION_CALLED_IN_OWNED_SCOPE: { legal: ["handler", "timer", "then", "micro"], recallContexts: [...OWNER_LIFETIME, ...FALSE_CLEAN_CONTEXTS, "cleanup-root-named", "memo-shared"],
    contexts: OWNED_PURE, bindings: ["direct", "alias", "destructured", "wrapper"],
    setup: (variant, name) => { const target = signal(name("w"), 0, "probe"); return [target, actionSignal(name("a"), target.name, 7)]; },
    call: (node, [, act]) => local(act.name),
  },
  RESOLVE_IN_REACTIVE_SCOPE: { legal: ["handler", "timer"], recallContexts: ["memo-shared"], contexts: ["memo", "render-compute"], bindings: BINDINGS, setup: () => [], call: () => api("resolve", "() => 1") },
  UNTIL_IN_REACTIVE_SCOPE: { legal: ["handler", "timer"], recallContexts: ["memo-shared"], contexts: ["memo", "render-compute"], bindings: BINDINGS, setup: () => [], call: () => api("until", "() => 1") },
  FLUSH_IN_ACTION: { legal: ["handler", "timer"], recallContexts: ["action-after-yield", "action-shared"], contexts: ["action"], bindings: BINDINGS, setup: () => [], call: () => api("flush") },
  FLUSH_IN_EFFECT_CALLBACK: { legal: ["handler", "timer"], recallContexts: ["effect-apply-rerun", "effect-apply-shared", "evt-apply-dispatch", "evt-apply-helper-dispatch", "evt-apply-window-dispatch", "evt-anchor-apply-listener", ...ANCHOR_MISSES, ...DENIED_FLUSH], recallLegal: ["evt-handler-dispatch", "evt-created-anchor", "timer-from-apply", "micro-from-apply"], contexts: ["effect-apply"], bindings: BINDINGS, setup: () => [], call: () => api("flush") },
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: { legal: ["handler", "timer"], recallContexts: ["tracked-rerun", "tracked-shared"], contexts: LEAF, bindings: BINDINGS, setup: () => [], call: () => api("flush") },
  DYNAMIC_STATIC_PROMISE: {
    contexts: ["body", "root", "untrack", "memo"], bindings: BINDINGS, setup: () => [], recallVariants: Object.keys(STATIC_SOURCES),
    call: (node) => api("dynamic", STATIC_SOURCES[node.variant] ?? '() => Promise.resolve("p" as const), { static: true }', { module: "@solidjs/web" }),
  },
};
// The flip's shapes (flip-read.mjs, flip-write.mjs): one file per track, so neither edits this table.
for (const contexts of [FLIP_READ_CONTEXTS, FLIP_WRITE_CONTEXTS]) {
  for (const [code, added] of Object.entries(contexts)) SPEC[code].recallContexts = [...(SPEC[code].recallContexts ?? []), ...added];
}
for (const [code, added] of Object.entries(FLIP_READ_LEGAL)) SPEC[code].recallLegal = [...(SPEC[code].recallLegal ?? []), ...added];
for (const [code, variants] of Object.entries(FLIP_READ_VARIANTS)) {
  const spec = SPEC[code];
  const setup = spec.setup;
  spec.setup = (variant, name) => (variants[variant] ? [openSignal(name("a"), variants[variant].kind)] : setup(variant, name));
  // A variant with `text` writes its own site over the source.
  const [call, bindingsFor] = [spec.call, spec.bindingsFor];
  spec.call = (node, sigs, ...rest) => (variants[node.variant]?.text ? member(sigs[0].name, variants[node.variant].text, variants[node.variant].imports, variants[node.variant]) : call(node, sigs, ...rest));
  spec.bindingsFor = (variant) => (variants[variant]?.text ? ["direct", "alias", "wrapper"] : bindingsFor?.(variant) ?? spec.bindings);
  const legal = Object.entries(variants).filter(([, { legal }]) => legal);
  spec.recallLegalVariants = [...(spec.recallLegalVariants ?? []), ...legal.map(([variant]) => variant)];
  spec.legalVariantContexts = { ...spec.legalVariantContexts, ...Object.fromEntries(legal.map(([variant, { legal }]) => [variant, legal])) };
  spec.recallVariants = [...(spec.recallVariants ?? []), ...Object.keys(variants)];
  spec.variantContexts = { ...spec.variantContexts, ...Object.fromEntries(Object.entries(variants).map(([variant, { contexts, legal = [] }]) => [variant, [...contexts, ...legal]])) };
}

/**
 * The rc.17 API variants (recall grids only), per code: each measured on the fast runtime before it got cells (2026-11-03), misuse
 * where its code reported at the site, legal where nothing was reported and the site ran.
 */
export const API_VARIANTS = {
  REACTIVE_WRITE_IN_OWNED_SCOPE: ["writable-memo", "optimistic", "owned-write"],
  STORE_WRITE_OUTSIDE_SETTER: ["optimistic-store", "projection", "optimistic-store-draft"],
  STRICT_READ_UNTRACKED: ["latest", "is-pending"],
  PENDING_ASYNC_UNTRACKED_READ: ["deferred"],
  PENDING_ASYNC_FORBIDDEN_SCOPE: ["deferred"],
  PRIMITIVE_IN_FORBIDDEN_SCOPE: ["signal-fn", "projection"],
  PRIMITIVE_IN_EFFECT_CALLBACK: ["signal-fn", "projection"],
  CONTEXT_NOT_FOUND: ["default-undefined"],
};

/**
 * The variants typed away on purpose, per code (recall grids only): every program of one is one `tsrs` rejects, and the grid judges the
 * checker's answer on it (tsc-overlap.mjs: a violation on the rejected code is `tsc-overlap`, never dropped). misuse.test.mjs checks
 * that each is rejected.
 */
export const TYPED_VARIANTS = {
  STORE_WRITE_OUTSIDE_SETTER: ["optional-link", "optional-link-leaf", "async-setter", "optional-link-draft"],
  STRICT_READ_UNTRACKED: ["uncalled-arith", "uncalled-index"],
  MISSING_EFFECT_FN: ["one-arg", "spread-one"],
};

/**
 * The ref contexts (`--recall` grids only; catalogue rank 1, zero coverage before): the site in a `ref` callback on an intrinsic.
 * rc.17's `ref` (`@solidjs/web`) runs the callback while the element is created, under `runWithOwner(null)`: no owner, so an owner
 * operation there is NO_OWNER_*, and a signal write or an action call is legal. `ref-factory` is a two-phase directive
 * (`ref={d()}`): the site in the apply `d` returns, which runs like an inline callback; `ref-factory-body` is the site in `d`'s own
 * body, which the component body calls, so it runs owned there. `ref-row` is an inline callback in a `For` row.
 */
export const REF_CONTEXTS = ["ref", "ref-named", "ref-array", "ref-cond", "ref-factory", "ref-row"];
/** Per code: the ref contexts that are its misuse cells (`misuse`) and its legal cells (`legal`). */
const REF_PLACEMENT = {
  STRICT_READ_UNTRACKED: { misuse: [...REF_CONTEXTS, "ref-factory-body"] },
  PENDING_ASYNC_UNTRACKED_READ: { misuse: REF_CONTEXTS },
  STORE_WRITE_OUTSIDE_SETTER: { misuse: [...REF_CONTEXTS, "ref-factory-body"] },
  NO_OWNER_CLEANUP: { misuse: REF_CONTEXTS, legal: ["ref-factory-body"] },
  NO_OWNER_EFFECT: { misuse: REF_CONTEXTS, legal: ["ref-factory-body"] },
  NO_OWNER_BOUNDARY: { misuse: REF_CONTEXTS, legal: ["ref-factory-body"] },
  NO_OWNER_CONTEXT: { misuse: REF_CONTEXTS, legal: ["ref-factory-body"] },
  SETTLED_CLEANUP_UNOWNED: { misuse: REF_CONTEXTS, legal: ["ref-factory-body"] },
  REACTIVE_WRITE_IN_OWNED_SCOPE: { misuse: ["ref-factory-body"], legal: REF_CONTEXTS },
  ACTION_CALLED_IN_OWNED_SCOPE: { misuse: ["ref-factory-body"], legal: REF_CONTEXTS },
};
/**
 * The owned and legal positions the base contexts miss (`--recall` only; catalogue ranks 5 and 10), each measured on rc.17's fast
 * runtime first (2026-11-01, 165 probes): a JSX attribute expression (compiled into a render effect: owned and tracked); a class
 * static block, field initializer, constructor and method run in the body (`new K()`, `new K().m()`); a body `forEach` callback;
 * a root, a bound owner or `untrack` inside a handler. rc.17's owner and window there:
 *   pos-attr            owned, tracked, no window: a write or an action throws, a read and `onCleanup` are silent
 *   pos-class-*, pos-foreach   the component body's (owned, labelled): a write, an action and a read report, `onCleanup` registers
 *   pos-handler-root, pos-handler-owner   a root (a bound owner), unlabelled: a write and an action throw, a read and `onCleanup` are silent
 *   pos-handler-untrack the handler's (no owner, no window): a write and an action are legal, `onCleanup` warns
 * A store proxy write warns in each. Existing contexts gain cells there too: a write or an action in an `onSettled` or tracked
 * effect block, under `runWithOwner(null)` and in an action body is legal (no children-capable owner); a read under
 * `runWithOwner(null)` warns (the body's label survives the owner change) and `onCleanup` in an action body warns (no owner).
 */
export const POS_CONTEXTS = ["pos-attr", "pos-class-sblock", "pos-class-field", "pos-class-ctor", "pos-class-method", "pos-foreach", "pos-handler-expr", "pos-flush-fn", "pos-handler-root", "pos-handler-owner", "pos-handler-untrack"];
// `pos-handler-expr` (the expression of `onClick={(…, h)}`, evaluated at the element's setup) and `pos-flush-fn` (`flush(fn)`) run in
// the body too.
const POS_BODY = ["pos-class-sblock", "pos-class-field", "pos-class-ctor", "pos-class-method", "pos-foreach", "pos-handler-expr", "pos-flush-fn"];
const POS_OWNED = ["pos-attr", ...POS_BODY, "pos-handler-root", "pos-handler-owner"];
const POS_PLACEMENT = {
  REACTIVE_WRITE_IN_OWNED_SCOPE: { misuse: POS_OWNED, legal: ["pos-handler-untrack", "settled", "tracked", "owner-null", "action"] },
  ACTION_CALLED_IN_OWNED_SCOPE: { misuse: POS_OWNED, legal: ["pos-handler-untrack", "settled", "tracked", "owner-null", "action"] },
  STORE_WRITE_OUTSIDE_SETTER: { misuse: [...POS_CONTEXTS, "settled", "tracked", "owner-null", "action"] },
  STRICT_READ_UNTRACKED: { misuse: [...POS_BODY, "owner-null"], legal: ["pos-attr", "pos-handler-root", "pos-handler-owner", "pos-handler-untrack", "action"] },
  NO_OWNER_CLEANUP: { misuse: ["pos-handler-untrack", "action", "action-after-yield"], legal: ["pos-attr", ...POS_BODY, "pos-handler-root", "pos-handler-owner"] },
  // An action body runs on its caller's stack (`@solidjs/signals` dist/dev.js:1573-1607), a handler here: no owner (ADR 0520).
  NO_OWNER_EFFECT: { misuse: ["action", "action-after-yield"] },
  NO_OWNER_BOUNDARY: { misuse: ["action", "action-after-yield"] },
  NO_OWNER_CONTEXT: { misuse: ["action", "action-after-yield"] },
  SETTLED_CLEANUP_UNOWNED: { misuse: ["action", "action-after-yield"] },
};
/**
 * The flow callback bodies (`--recall` only; catalogue rank 2): the site in the body of a flow component's function child,
 * `<For each={[1]}>{() => { …; return <li/>; }}</For>`. Measured on rc.17 (2026-11-01, 168 programs, Happy DOM and Chrome alike):
 * `For` (keyed or not) and `Repeat` run the body in a fresh row root, untracked, under the label "<For>"/"<Repeat>"; a
 * one-parameter `Show`/`Match` child runs under `untrack(child, "<Show>")` in the flow's memo. So a read warns, a pending read
 * throws, a write or an action throws (an owner), a store proxy write warns, and an owner operation is legal.
 */
export const FLOW_CONTEXTS = FLOW_FORMS.map((form) => `flow-${form}`);
const FLOW_PLACEMENT = {
  STRICT_READ_UNTRACKED: { misuse: FLOW_CONTEXTS },
  PENDING_ASYNC_UNTRACKED_READ: { misuse: FLOW_CONTEXTS },
  REACTIVE_WRITE_IN_OWNED_SCOPE: { misuse: FLOW_CONTEXTS },
  ACTION_CALLED_IN_OWNED_SCOPE: { misuse: FLOW_CONTEXTS },
  STORE_WRITE_OUTSIDE_SETTER: { misuse: FLOW_CONTEXTS },
  NO_OWNER_CLEANUP: { legal: FLOW_CONTEXTS },
  NO_OWNER_EFFECT: { legal: FLOW_CONTEXTS },
  NO_OWNER_CONTEXT: { legal: FLOW_CONTEXTS },
};
/**
 * The callback props (`--recall` only; catalogue rank 3, section 3.11): C1 hands the site's function to a same-file child as a prop,
 * `<KidN run={() => { …site… }} />`, and the child calls `props.run()` in its body, its JSX, a click handler, an effect apply,
 * `onSettled`, or never (model.mjs CBP_CALLS); `run` is written inline, named by C1 (`function runN() {}`), forwarded by a middle
 * component, destructured by the child, or spread by a middle component (CBP_FORMS). `cbp-<call>` is the inline form,
 * `cbp-<call>-<form>` the others. Measured on rc.17's fast runtime first (2026-11-01, 372 programs, probe in
 * /tmp/sc-scratch.noindex/callback-props; the five forms agree on every site): the call runs where the child places it.
 *   body      the child's body (owned, labelled "<KidN>"): a write, a refresh or an action throws, a read warns, a pending read
 *             throws, a store proxy write warns, an owner operation is legal, a no-default `useContext` reports CONTEXT_NOT_FOUND
 *   jsx       a render effect (owned, tracked): a write or an action throws, a read is silent, a pending read is
 *             ASYNC_OUTSIDE_LOADING_BOUNDARY, a store proxy write warns, an owner operation is legal, CONTEXT_NOT_FOUND
 *   handler   no owner: a write, an action and a read are legal; `onCleanup`, an effect, a boundary, `useContext` and `onSettled`
 *             report NO_OWNER_*; a store proxy write warns
 *   apply     an effect apply (no owner, strict-read window open): a write and an action are legal; a read warns, a pending read
 *             throws; `onCleanup`, a boundary, `useContext`, `onSettled` report NO_OWNER_*; a primitive is PRIMITIVE_IN_EFFECT_CALLBACK,
 *             `flush()` FLUSH_IN_EFFECT_CALLBACK
 *   settled   onSettled's leaf owner: a write, an action, a read and `useContext` are legal; `onCleanup` is CLEANUP_IN_FORBIDDEN_SCOPE,
 *             a primitive or boundary PRIMITIVE_IN_FORBIDDEN_SCOPE, a pending read PENDING_ASYNC_FORBIDDEN_SCOPE, `flush()`
 *             FLUSH_IN_TRACKED_QUEUE_CALLBACK, `onSettled` SETTLED_CLEANUP_UNOWNED, CONTEXT_NOT_FOUND
 *   never     the site never runs: legal for every code
 * Not cells: an effect in an apply (PRIMITIVE_IN_EFFECT_CALLBACK, not NO_OWNER_EFFECT) or in onSettled, a boundary in onSettled,
 * a pending read in JSX or onSettled for PENDING_ASYNC_UNTRACKED_READ (another code fires there, so neither misuse nor legal).
 */
export const CBP_CONTEXTS = CBP_CALLS.flatMap((call) => CBP_FORMS.map((form) => (form === "inline" ? `cbp-${call}` : `cbp-${call}-${form}`)));
// `attached` (CBP_CALLS): the child's intrinsic runs `props.run` itself as its click handler, as `handler` does through an arrow.
const cbp = (...calls) => CBP_CONTEXTS.filter((context) => calls.flatMap((call) => (call === "handler" ? ["handler", "attached"] : [call])).includes(/^cbp-([a-z]+)/.exec(context)[1]));
const CBP_PLACEMENT = {
  REACTIVE_WRITE_IN_OWNED_SCOPE: { misuse: cbp("body", "jsx"), legal: cbp("handler", "apply", "settled", "never") },
  ACTION_CALLED_IN_OWNED_SCOPE: { misuse: cbp("body", "jsx"), legal: cbp("handler", "apply", "settled", "never") },
  STORE_WRITE_OUTSIDE_SETTER: { misuse: cbp("body", "jsx", "handler", "apply", "settled"), legal: cbp("never") },
  STRICT_READ_UNTRACKED: { misuse: cbp("body", "apply"), legal: cbp("jsx", "handler", "settled", "never") },
  PENDING_ASYNC_UNTRACKED_READ: { misuse: cbp("body", "apply"), legal: cbp("handler", "never") },
  NO_OWNER_CLEANUP: { misuse: cbp("handler", "apply"), legal: cbp("body", "jsx", "never") },
  NO_OWNER_EFFECT: { misuse: cbp("handler"), legal: cbp("body", "jsx", "never") },
  NO_OWNER_BOUNDARY: { misuse: cbp("handler", "apply"), legal: cbp("body", "jsx", "never") },
  NO_OWNER_CONTEXT: { misuse: cbp("handler", "apply"), legal: cbp("body", "jsx", "settled", "never") },
  SETTLED_CLEANUP_UNOWNED: { misuse: cbp("handler", "apply", "settled"), legal: cbp("body", "jsx", "never") },
  CLEANUP_IN_FORBIDDEN_SCOPE: { misuse: cbp("settled"), legal: cbp("body", "jsx", "never") },
  PRIMITIVE_IN_FORBIDDEN_SCOPE: { misuse: cbp("settled"), legal: cbp("body", "jsx", "never") },
  PRIMITIVE_IN_EFFECT_CALLBACK: { misuse: cbp("apply"), legal: cbp("body", "jsx", "never") },
  PENDING_ASYNC_FORBIDDEN_SCOPE: { misuse: cbp("settled"), legal: cbp("handler", "never") },
  ASYNC_OUTSIDE_LOADING_BOUNDARY: { misuse: cbp("jsx"), legal: cbp("handler", "never") },
  FLUSH_IN_EFFECT_CALLBACK: { misuse: cbp("apply"), legal: cbp("body", "jsx", "handler", "never") },
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: { misuse: cbp("settled"), legal: cbp("body", "jsx", "handler", "never") },
  CONTEXT_NOT_FOUND: { misuse: cbp("body", "jsx", "settled"), legal: cbp("never") },
};
/**
 * The effect windows (`--recall` only; catalogue rank 4): the site in an effect's returned cleanup when the effect reruns
 * (`effect-cleanup-rerun`, `render-cleanup-rerun`: the compute reads a signal a button sets to 2) and when its owner is disposed (a
 * root's `dispose()` from a button, a `Show` turned off, a `For` row removed: a button sets the condition to 0), in a user effect's
 * `error` arm (the compute throws once the button sets 2), in a render effect's apply on its rerun only (`render-apply-guarded-rerun`)
 * and in a `createReaction` callback. `effect-cleanup-rerun-named` names the apply by a binding (ADR 0539: the checker read only
 * inline applies' cleanups). Measured on rc.17's fast runtime first (2026-11-01, 247 probes; `@solidjs/signals`
 * dist/dev.js, dev-shared.js):
 *   cleanup on a rerun, render apply rerun, reaction   `runEffect` (dev.js:1991-2026) runs the previous cleanup and the apply in the
 *                       effect-callback window (`setStrictRead("an effect callback")`, `setEffectCallback(true)`) with no owner (the
 *                       flush): a read warns, a primitive or flush reports, an owner operation is NO_OWNER_*, a write or an action is legal
 *   cleanup on a root's dispose or a Show turned off   `disposeChildren` (dev-shared.js:1287-1293) under `enterDisposal`, no owner and
 *                       no window: a read, a write, an action, a primitive and flush are silent; an owner operation is NO_OWNER_*
 *   cleanup on a For row removed   the row is disposed inside `runWithOwner(this._owner)` (dev.js:3347): owned and under disposal, so a
 *                       write is exempt (dev-shared.js:5621-5626) but an action throws (its guard has no disposal exemption, dev.js:1535)
 *                       and an owner operation is silent
 *   error arm           `_errorFn` runs from `runEffect` (dev.js:1966-1989) outside the window and with no owner: like a root's dispose
 * A store proxy write warns in each. A `createReaction` callback runs as its effect's apply (dev.js:2653-2664); its `error` handler is
 * the effect's error arm, the same as `effect-error-arm`.
 */
export const EFFECT_WINDOW_CONTEXTS = ["effect-cleanup-rerun", "render-cleanup-rerun", "effect-cleanup-rerun-named", "effect-cleanup-dispose", "effect-cleanup-show", "effect-cleanup-for",
  "effect-error-arm", "render-apply-guarded-rerun", "reaction", "reaction-error-arm"];
const WINDOWED = ["effect-cleanup-rerun", "render-cleanup-rerun", "effect-cleanup-rerun-named", "render-apply-guarded-rerun", "reaction"];
const UNWINDOWED = ["effect-cleanup-dispose", "effect-cleanup-show", "effect-error-arm", "reaction-error-arm"];
const ROW_DISPOSAL = ["effect-cleanup-for"];
const EFFECT_WINDOW_PLACEMENT = {
  STRICT_READ_UNTRACKED: { misuse: WINDOWED, legal: [...UNWINDOWED, ...ROW_DISPOSAL] },
  REACTIVE_WRITE_IN_OWNED_SCOPE: { legal: EFFECT_WINDOW_CONTEXTS },
  ACTION_CALLED_IN_OWNED_SCOPE: { misuse: ROW_DISPOSAL, legal: [...WINDOWED, ...UNWINDOWED] },
  STORE_WRITE_OUTSIDE_SETTER: { misuse: EFFECT_WINDOW_CONTEXTS },
  NO_OWNER_CLEANUP: { misuse: [...WINDOWED, ...UNWINDOWED], legal: ROW_DISPOSAL },
  // In the window an effect is PRIMITIVE_IN_EFFECT_CALLBACK, not this code: neither cell.
  NO_OWNER_EFFECT: { misuse: UNWINDOWED, legal: ROW_DISPOSAL },
  NO_OWNER_BOUNDARY: { misuse: [...WINDOWED, ...UNWINDOWED], legal: ROW_DISPOSAL },
  NO_OWNER_CONTEXT: { misuse: [...WINDOWED, ...UNWINDOWED], legal: ROW_DISPOSAL },
  SETTLED_CLEANUP_UNOWNED: { misuse: [...WINDOWED, ...UNWINDOWED], legal: ROW_DISPOSAL },
  // Out of the window an unowned effect is NO_OWNER_EFFECT: the `effect` variant is not a legal program there (`recallVariantsNotIn`).
  PRIMITIVE_IN_EFFECT_CALLBACK: { misuse: WINDOWED, legal: [...UNWINDOWED, ...ROW_DISPOSAL] },
  FLUSH_IN_EFFECT_CALLBACK: { misuse: WINDOWED, legal: [...UNWINDOWED, ...ROW_DISPOSAL] },
};
SPEC.PRIMITIVE_IN_EFFECT_CALLBACK.recallVariantsNotIn = Object.fromEntries(UNWINDOWED.map((context) => [context, ["effect"]]));
/**
 * The action step (`--recall` only; Fable's precision report 2026-11-02, section 3): the site in an action's first synchronous slice,
 * the action called from an effect apply, a `createReaction` callback, an owned `onSettled`, a `createTrackedEffect` or an effect's
 * `error` arm, and after a plain-value `yield 1` from an apply. rc.17's `step` (`@solidjs/signals` dist/dev.js:1573-1600) sets the
 * action depth and clears the effect-callback marker; the owner, the tracking state and the strict-read window are the caller's,
 * and a plain-value `yield` resumes synchronously on the same stack (dev.js:1637). Measured on the fast runtime (2026-11-02):
 *   from an apply, a reaction callback, after `yield 1` from an apply   no owner, window "an effect callback": a read warns, an owner
 *                       operation is NO_OWNER_*, a memo or root is legal (the marker is cleared), a write or an action is legal
 *   from an owned onSettled or a tracked effect   the leaf owner: a memo, root or effect is PRIMITIVE_IN_FORBIDDEN_SCOPE, onCleanup
 *                       CLEANUP_IN_FORBIDDEN_SCOPE; a read, a write and an action are legal
 *   from an error arm   no owner, no window: a read, a write and an action are legal; an owner operation is NO_OWNER_*
 * `flush` throws FLUSH_IN_ACTION in each (SC2006's, already a cell through `action`).
 */
export const ACTION_STEP_CONTEXTS = ["action-from-apply", "action-from-reaction", "action-plain-yield-from-apply", "action-from-settled", "action-from-tracked", "action-from-error-arm"];
const ACTION_WINDOWED = ["action-from-apply", "action-from-reaction", "action-plain-yield-from-apply"];
const ACTION_LEAF = ["action-from-settled", "action-from-tracked"];
const ACTION_STEP_PLACEMENT = {
  STRICT_READ_UNTRACKED: { misuse: ACTION_WINDOWED, legal: [...ACTION_LEAF, "action-from-error-arm"] },
  PRIMITIVE_IN_FORBIDDEN_SCOPE: { misuse: ACTION_LEAF },
  CLEANUP_IN_FORBIDDEN_SCOPE: { misuse: ACTION_LEAF },
  REACTIVE_WRITE_IN_OWNED_SCOPE: { legal: ACTION_STEP_CONTEXTS },
  ACTION_CALLED_IN_OWNED_SCOPE: { legal: ACTION_STEP_CONTEXTS },
  NO_OWNER_CLEANUP: { misuse: [...ACTION_WINDOWED, "action-from-error-arm"] },
  NO_OWNER_EFFECT: { misuse: [...ACTION_WINDOWED, "action-from-error-arm"] },
  NO_OWNER_BOUNDARY: { misuse: [...ACTION_WINDOWED, "action-from-error-arm"] },
  NO_OWNER_CONTEXT: { misuse: [...ACTION_WINDOWED, "action-from-error-arm"] },
  // The marker is cleared for the step: a memo or root in the body is legal where the apply itself reports.
  PRIMITIVE_IN_EFFECT_CALLBACK: { legal: ACTION_WINDOWED },
};
/**
 * The thunk contexts (`--recall` only; ADR 0551): `resolve(fn)` and `until(fn)` run `fn` as the compute of a `computed` under a fresh
 * root (`@solidjs/signals` rc.17 dist/dev.js:2694-2770, :2909), measured in Fable's precision probes (row 43): a write or an action
 * throws, a read, a root, a cleanup and an effect are legal.
 */
const THUNK_CONTEXTS = ["resolve-thunk", "until-thunk"];
const THUNK_PLACEMENT = {
  REACTIVE_WRITE_IN_OWNED_SCOPE: { misuse: THUNK_CONTEXTS },
  ACTION_CALLED_IN_OWNED_SCOPE: { misuse: THUNK_CONTEXTS },
  STRICT_READ_UNTRACKED: { legal: THUNK_CONTEXTS },
  NO_OWNER_CLEANUP: { legal: THUNK_CONTEXTS },
  NO_OWNER_EFFECT: { legal: THUNK_CONTEXTS },
};
/**
 * The later fire sites, named handlers and never-run code (`--recall` only, base shapes only; Fable's report 2026-11-03, R2-R4): each
 * is equivalent at run time to the base context it names (a function a helper defers to a timer runs as a timer callback does), so a
 * code takes it as a misuse cell where that context is one of its misuse contexts and as a legal cell where it is legal; measured on
 * the fast runtime before (2026-11-03). `handler-mixed` (a named handler the body also calls) is a misuse where either caller is.
 */
export const FIRE_CONTEXTS = { "fire-helper-timer": "timer", "fire-helper-micro": "micro", "fire-helper-then": "then", "fire-store-handler": "handler", "fire-store-body": "body", "fire-store-timer": "timer", "fire-return-handler": "handler", "fire-return-body": "body" };
export const HANDLER_CONTEXTS = { "handler-named-const": "handler", "handler-named-function": "handler", "handler-tuple": "handler" };
export const DEAD_CONTEXTS = ["dead-if", "dead-after-return", "dead-and-false", "dead-comp-never"];
const EQUIVALENT_PLACEMENT = Object.fromEntries(Object.keys(SPEC).filter((code) => !SPEC[code].typedAway && !SPEC[code].recallOnly).map((code) => {
  const spec = SPEC[code];
  const placed = (kind) => Object.entries({ ...FIRE_CONTEXTS, ...HANDLER_CONTEXTS }).filter(([, base]) => (kind === "misuse" ? spec.contexts : spec.legal ?? []).includes(base)).map(([context]) => context);
  const mixed = spec.contexts.includes("body") || spec.contexts.includes("handler") ? "misuse" : (spec.legal ?? []).includes("body") && (spec.legal ?? []).includes("handler") ? "legal" : null;
  return [code, { misuse: [...placed("misuse"), ...(mixed === "misuse" ? ["handler-mixed"] : [])], legal: [...placed("legal"), ...(mixed === "legal" ? ["handler-mixed"] : []), ...DEAD_CONTEXTS] }];
}));
/** Contexts that take their base shapes only (matrix.mjs `recallAxes`): their question is the host's run, not the spelling at the site. */
export const BASE_ONLY_CONTEXTS = () => [...API_CONTEXTS, ...Object.keys(FIRE_CONTEXTS), ...Object.keys(HANDLER_CONTEXTS), "handler-mixed", ...DEAD_CONTEXTS];

/**
 * ADR 0613: the site in an effect apply, with a synchronous `flush()` the program calls after the effect is created: in the body
 * (`-body`), a memo compute (`-memo`) or a root body holding both (`-root`), where rc.17 drains the apply under that owner, and in a
 * handler (`-handler`), where the drain has no owner (the apply's own first run is the mount's, before the click). Base shapes only.
 * `FLUSH_RERUN_CONTEXTS`: what only a rerun runs (an apply's returned cleanup, a render effect's later apply, an `error` arm, a
 * reaction's callback, a deferred apply), the button writing the trigger and draining it in a `createRoot` body.
 */
export const FLUSH_RERUN_CONTEXTS = ["effect-cleanup-flush-rerun", "render-cleanup-flush-rerun", "render-apply-flush-rerun", "effect-error-arm-flush-rerun", "reaction-flush-rerun", "effect-apply-defer-flush-rerun"];
export const FLUSH_DRAIN_CONTEXTS = ["effect-apply-flush-body", "effect-apply-flush-memo", "effect-apply-flush-root", "effect-apply-flush-handler", "settled-flush-root", ...FLUSH_RERUN_CONTEXTS];
/** The lazy-memo, deferred-apply and flush-drain contexts (`--recall` only, base shapes only: API_CONTEXTS above `recallContext`'s cases). */
export const API_CONTEXTS = ["memo-lazy-unread", "memo-lazy-read-handler", "effect-apply-defer", "effect-apply-defer-rerun", ...FLUSH_DRAIN_CONTEXTS];
/**
 * Measured on rc.17's fast runtime (2026-11-03, 162 probes, every variant of each code): a site in a lazy memo nobody reads, or in a
 * deferred apply with no rerun, never runs (legal for every code: a violation there is an unexecuted one, ADR 0458). A lazy memo read
 * in a handler computes there as its own computation: a write, a store proxy write, an action call and a sync node handed async
 * report; an owner operation, a read and a primitive are silent (CONTEXT_NOT_FOUND, resolve, until and a static dynamic source are
 * reported off the site's line: neither cell). A deferred apply's rerun is an apply's: the codes of `effect-apply-rerun` report, a
 * write or an action is legal.
 */
const LAZY_HANDLER_LEGAL = ["STRICT_READ_UNTRACKED", "NO_OWNER_CLEANUP", "NO_OWNER_EFFECT", "NO_OWNER_BOUNDARY", "NO_OWNER_CONTEXT", "SETTLED_CLEANUP_UNOWNED", "CLEANUP_IN_FORBIDDEN_SCOPE", "PRIMITIVE_IN_FORBIDDEN_SCOPE",
  "PENDING_ASYNC_UNTRACKED_READ", "PENDING_ASYNC_FORBIDDEN_SCOPE", "UNTRACKED_READ_AFTER_AWAIT", "FLUSH_IN_ACTION", "FLUSH_IN_EFFECT_CALLBACK", "FLUSH_IN_TRACKED_QUEUE_CALLBACK", "PRIMITIVE_IN_EFFECT_CALLBACK"];
const LAZY_HANDLER_MISUSE = ["REACTIVE_WRITE_IN_OWNED_SCOPE", "STORE_WRITE_OUTSIDE_SETTER", "SYNC_NODE_RECEIVED_ASYNC", "ACTION_CALLED_IN_OWNED_SCOPE"];
const DEFER_RERUN_MISUSE = ["STRICT_READ_UNTRACKED", "NO_OWNER_CLEANUP", "NO_OWNER_BOUNDARY", "NO_OWNER_CONTEXT", "SETTLED_CLEANUP_UNOWNED", "STORE_WRITE_OUTSIDE_SETTER", "FLUSH_IN_EFFECT_CALLBACK", "PRIMITIVE_IN_EFFECT_CALLBACK", "DYNAMIC_STATIC_PROMISE"];
const DEFER_RERUN_LEGAL = ["REACTIVE_WRITE_IN_OWNED_SCOPE", "ACTION_CALLED_IN_OWNED_SCOPE"];
// The async family (2026-11-04, rc.17 fast runtime, 476 probes: every base variant of each code in each context, direct, no
// layer; a context takes a cell only where every variant agreed): past an await in an async function the body calls (`async`,
// its loop, `for await` and `finally`), in an async memo or effect compute (`memo-async*`, `effect-async*`), or in a handler's
// async function (`handler-async-loop`, `-finally`). A cell a code already had is unchanged; mixed or disturbed answers take none
// (STRICT_READ_UNTRACKED and the pending codes in an async compute report UNTRACKED_READ_AFTER_AWAIT, CLEANUP_IN_FORBIDDEN_SCOPE
// reports another code everywhere, CONTEXT_NOT_FOUND outside a compute throws). Every legal cell was measured again with its site in
// a `try` and a marker after it: a code an async compute throws past its await only rejects the compute's promise and no diagnostic
// says so, so `useContext` (NO_OWNER_CONTEXT, CONTEXT_NOT_FOUND) and `dynamic(..., { static: true })` in `memo-async*`/`effect-async*`
// are neither legal (they throw) nor misuse cells (nothing reports them): they take no cell.
const PLAIN_ASYNC = ["async", ...ASYNC_CONTROL];
const COMPUTE_ASYNC = ["memo-async", "effect-async", "memo-async-loop", "memo-async-for-await", "memo-async-finally", "effect-async-loop", "effect-async-for-await", "effect-async-finally"];
const HANDLER_ASYNC_PROBED = ["handler-async-loop", "handler-async-finally"];
/** Contexts where a code the runtime only throws rejects a promise nothing logs, so its silence is no measure (the comment above; matrix.mjs censusCells). */
export const PROMISE_SWALLOWS = [...COMPUTE_ASYNC, "resolve-thunk", "until-thunk"];
const ASYNC_FAMILY_PLACEMENT = {
  STRICT_READ_UNTRACKED: { legal: [...PLAIN_ASYNC, ...HANDLER_ASYNC_PROBED] },
  PENDING_ASYNC_UNTRACKED_READ: { legal: [...PLAIN_ASYNC, ...HANDLER_ASYNC_PROBED] },
  PENDING_ASYNC_FORBIDDEN_SCOPE: { legal: [...PLAIN_ASYNC, ...HANDLER_ASYNC_PROBED] },
  UNTRACKED_READ_AFTER_AWAIT: { legal: [...PLAIN_ASYNC, ...HANDLER_ASYNC_PROBED] },
  NO_OWNER_CONTEXT: { misuse: HANDLER_ASYNC_PROBED },
  DYNAMIC_STATIC_PROMISE: { misuse: [...PLAIN_ASYNC, ...HANDLER_ASYNC_PROBED] },
  ...Object.fromEntries(["NO_OWNER_CLEANUP", "NO_OWNER_EFFECT", "NO_OWNER_BOUNDARY", "SETTLED_CLEANUP_UNOWNED", "STORE_WRITE_OUTSIDE_SETTER"].map((code) => [code, { misuse: [...COMPUTE_ASYNC, ...HANDLER_ASYNC_PROBED] }])),
  SYNC_NODE_RECEIVED_ASYNC: { misuse: [...PLAIN_ASYNC, ...COMPUTE_ASYNC, ...HANDLER_ASYNC_PROBED] },
  ...Object.fromEntries(["ACTION_CALLED_IN_OWNED_SCOPE", "RESOLVE_IN_REACTIVE_SCOPE", "UNTIL_IN_REACTIVE_SCOPE", "FLUSH_IN_ACTION", "FLUSH_IN_EFFECT_CALLBACK", "FLUSH_IN_TRACKED_QUEUE_CALLBACK"].map((code) => [code, { legal: [...PLAIN_ASYNC, ...COMPUTE_ASYNC, ...HANDLER_ASYNC_PROBED] }])),
};
const API_CONTEXT_PLACEMENT = Object.fromEntries(Object.keys(SPEC).filter((code) => !SPEC[code].typedAway && !SPEC[code].recallOnly).map((code) => [code, {
  misuse: [...(LAZY_HANDLER_MISUSE.includes(code) ? ["memo-lazy-read-handler"] : []), ...(DEFER_RERUN_MISUSE.includes(code) ? ["effect-apply-defer-rerun"] : [])],
  legal: ["memo-lazy-unread", "effect-apply-defer", ...(LAZY_HANDLER_LEGAL.includes(code) ? ["memo-lazy-read-handler"] : []), ...(DEFER_RERUN_LEGAL.includes(code) ? ["effect-apply-defer-rerun"] : [])],
}]));
/**
 * ADR 0613, measured on rc.17's fast runtime (2026-11-04, 340 probes, every variant of each code; a cell where the variants disagree,
 * a disturbed run or another defect gets none): with the flush in the body, a memo or a root (or, for what a rerun runs, a root body in
 * the handler) the callback runs under that owner, so a write or an action call reports and the owner codes (NO_OWNER_*,
 * PRIMITIVE_IN_EFFECT_CALLBACK, SETTLED_CLEANUP_UNOWNED) are silent; with it in a handler the apply runs as any apply does.
 */
const FLUSH_DRAIN_MEASURED = {
  ACTION_CALLED_IN_OWNED_SCOPE: { misuse: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "memo", "reaction", "render-apply", "render-cleanup", "root"], legal: ["handler"] },
  CLEANUP_IN_FORBIDDEN_SCOPE: { misuse: [], legal: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "memo", "reaction", "render-apply", "render-cleanup", "root"] },
  CONTEXT_NOT_FOUND: { misuse: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "reaction", "render-apply", "render-cleanup", "root"], legal: [] },
  DYNAMIC_STATIC_PROMISE: { misuse: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "handler", "reaction", "render-apply", "render-cleanup", "root"], legal: [] },
  FLUSH_IN_ACTION: { misuse: [], legal: ["effect-error-arm"] },
  FLUSH_IN_EFFECT_CALLBACK: { misuse: ["body", "effect-apply-defer", "effect-cleanup", "handler", "memo", "reaction", "render-apply", "render-cleanup", "root"], legal: ["effect-error-arm"] },
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: { misuse: [], legal: ["effect-error-arm"] },
  NO_OWNER_BOUNDARY: { misuse: ["handler"], legal: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "memo", "reaction", "render-apply", "render-cleanup", "root"] },
  NO_OWNER_CLEANUP: { misuse: ["handler"], legal: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "memo", "reaction", "render-apply", "render-cleanup", "root"] },
  NO_OWNER_CONTEXT: { misuse: ["handler"], legal: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "memo", "reaction", "render-apply", "render-cleanup", "root"] },
  NO_OWNER_EFFECT: { misuse: [], legal: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "memo", "reaction", "render-apply", "render-cleanup", "root"] },
  PENDING_ASYNC_FORBIDDEN_SCOPE: { misuse: [], legal: ["effect-error-arm"] },
  PENDING_ASYNC_UNTRACKED_READ: { misuse: ["body", "handler", "memo", "root"], legal: ["effect-error-arm"] },
  PRIMITIVE_IN_EFFECT_CALLBACK: { misuse: ["handler"], legal: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "memo", "reaction", "render-apply", "render-cleanup", "root"] },
  PRIMITIVE_IN_FORBIDDEN_SCOPE: { misuse: [], legal: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "memo", "reaction", "render-apply", "render-cleanup", "root"] },
  REACTIVE_WRITE_IN_OWNED_SCOPE: { misuse: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "memo", "reaction", "render-apply", "render-cleanup", "root"], legal: ["handler"] },
  RESOLVE_IN_REACTIVE_SCOPE: { misuse: [], legal: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "reaction", "render-apply", "render-cleanup", "root"] },
  SETTLED_CLEANUP_UNOWNED: { misuse: ["handler"], legal: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "memo", "reaction", "render-apply", "render-cleanup", "root"] },
  STORE_WRITE_OUTSIDE_SETTER: { misuse: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "handler", "memo", "reaction", "render-apply", "render-cleanup", "root"], legal: [] },
  STRICT_READ_UNTRACKED: { misuse: ["body", "effect-apply-defer", "effect-cleanup", "handler", "memo", "reaction", "render-apply", "render-cleanup", "root"], legal: ["effect-error-arm"] },
  SYNC_NODE_RECEIVED_ASYNC: { misuse: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "memo", "reaction", "render-apply", "render-cleanup", "root"], legal: [] },
  UNTIL_IN_REACTIVE_SCOPE: { misuse: [], legal: ["body", "effect-apply-defer", "effect-cleanup", "effect-error-arm", "reaction", "render-apply", "render-cleanup", "root"] },
  UNTRACKED_READ_AFTER_AWAIT: { misuse: [], legal: ["effect-error-arm"] },
};
const flushDrainContext = (at) => (["body", "memo", "root", "handler"].includes(at) ? `effect-apply-flush-${at}` : `${at}-flush-rerun`);
const FLUSH_DRAIN_PLACEMENT = Object.fromEntries(Object.entries(FLUSH_DRAIN_MEASURED).map(([code, { misuse, legal }]) => [code, { misuse: misuse.map(flushDrainContext), legal: legal.map(flushDrainContext) }]));
// `settled-flush-root` (2026-11-04, 34 probes on rc.17, every variant agreed): a handler's `onSettled` drained by a root's `flush()` runs
// under the root, not as a leaf: an owner operation and a primitive are legal, a write, a store proxy write, an action and a sync node
// handed async report, and a provider-free context read throws (ADR 0613).
for (const code of ["CONTEXT_NOT_FOUND", "REACTIVE_WRITE_IN_OWNED_SCOPE", "STORE_WRITE_OUTSIDE_SETTER", "SYNC_NODE_RECEIVED_ASYNC", "ACTION_CALLED_IN_OWNED_SCOPE", "DYNAMIC_STATIC_PROMISE"]) (FLUSH_DRAIN_PLACEMENT[code] ??= { misuse: [], legal: [] }).misuse.push("settled-flush-root");
for (const code of ["STRICT_READ_UNTRACKED", "NO_OWNER_CLEANUP", "NO_OWNER_EFFECT", "NO_OWNER_BOUNDARY", "NO_OWNER_CONTEXT", "SETTLED_CLEANUP_UNOWNED", "CLEANUP_IN_FORBIDDEN_SCOPE", "PRIMITIVE_IN_FORBIDDEN_SCOPE", "PENDING_ASYNC_UNTRACKED_READ", "PENDING_ASYNC_FORBIDDEN_SCOPE", "UNTRACKED_READ_AFTER_AWAIT", "RESOLVE_IN_REACTIVE_SCOPE", "UNTIL_IN_REACTIVE_SCOPE", "FLUSH_IN_ACTION", "FLUSH_IN_EFFECT_CALLBACK", "FLUSH_IN_TRACKED_QUEUE_CALLBACK", "PRIMITIVE_IN_EFFECT_CALLBACK"]) (FLUSH_DRAIN_PLACEMENT[code] ??= { misuse: [], legal: [] }).legal.push("settled-flush-root");
// `settled-handler`: a handler's `onSettled` with nothing to drain it but the queue. No leaf owner, and the source has settled by the click: a
// pending read reports nothing (rc.17, 2026-11-04). A helper's read was a proven SC5001 here, older than `settled-flush-root`, which first reached it.
for (const code of ["PENDING_ASYNC_UNTRACKED_READ", "PENDING_ASYNC_FORBIDDEN_SCOPE"]) (FLUSH_DRAIN_PLACEMENT[code] ??= { misuse: [], legal: [] }).legal.push("settled-handler");
// With no owner an effect in the step is NO_OWNER_EFFECT, not this code: the `effect` variant is no legal program there.
for (const context of ACTION_WINDOWED) SPEC.PRIMITIVE_IN_EFFECT_CALLBACK.recallVariantsNotIn[context] = ["effect"];
// Under an `<Errored>` mount the `NoOwnerError` an error arm throws escalates to the boundary, which logs the compute's own error
// instead (fast runtime, 2026-11-01): Solid never names this code there.
SPEC.NO_OWNER_CONTEXT.recallMountsNotIn = { "effect-error-arm": ["errored"], "reaction-error-arm": ["errored"] };
for (const placement of [REF_PLACEMENT, POS_PLACEMENT, FLOW_PLACEMENT, CBP_PLACEMENT, EFFECT_WINDOW_PLACEMENT, ACTION_STEP_PLACEMENT, THUNK_PLACEMENT, API_CONTEXT_PLACEMENT, EQUIVALENT_PLACEMENT, ASYNC_FAMILY_PLACEMENT, FLUSH_DRAIN_PLACEMENT]) for (const [code, { misuse = [], legal = [] }] of Object.entries(placement)) {
  const has = new Set([...SPEC[code].contexts, ...(SPEC[code].recallContexts ?? [])]);
  const hasLegal = new Set([...(SPEC[code].legal ?? []), ...(SPEC[code].recallLegal ?? [])]);
  SPEC[code].recallContexts = [...(SPEC[code].recallContexts ?? []), ...misuse.filter((context) => !has.has(context))];
  SPEC[code].recallLegal = [...(SPEC[code].recallLegal ?? []), ...legal.filter((context) => !hasLegal.has(context))];
}

// A legal variant whose legal places include contexts where the code's other variants are misuses (`legalVariantContexts`): those
// contexts become legal cells (`recallLegal`) that take only such variants (`variantOnlyLegal`, matrix.mjs `recallAxes`).
for (const [code, spec] of Object.entries(SPEC)) for (const [variant, contexts] of Object.entries(spec.legalVariantContexts ?? {})) for (const context of contexts) {
  if (spec.legal?.includes(context) || (spec.recallLegal?.includes(context) && !spec.variantOnlyLegal?.[context])) continue;
  spec.recallLegal = [...new Set([...(spec.recallLegal ?? []), context])];
  spec.variantOnlyLegal = { ...spec.variantOnlyLegal, [context]: [...(spec.variantOnlyLegal?.[context] ?? []), variant] };
}

/**
 * The codes the matrix measures, in the policy table's order: the defects, claimed by a rule or not ("never quieter than Solid": an
 * unclaimed defect the checker is silent on is a false clean too). An advisory code (Solid documents it as legal) is not measured.
 */
export const MISUSE_CODES = Object.keys(codePolicy).filter((code) => SPEC[code] && (SPEC[code].typedAway || SPEC[code].recallOnly || ["defect-claimed", "defect-unclaimed"].includes(policyOf(code)?.disposition)));
/** The codes that have a template (not typed away). */
export const TEMPLATE_CODES = MISUSE_CODES.filter((code) => !SPEC[code].typedAway && !SPEC[code].recallOnly);
/** The codes with a template only the recall grids measure (never drawn, so no other program moves). */
export const RECALL_TEMPLATE_CODES = MISUSE_CODES.filter((code) => SPEC[code].recallOnly);
export const contextsOf = (code) => SPEC[code].contexts;
/** The recall contexts of a code (`--recall` grids only). */
export const recallContextsOf = (code) => SPEC[code].recallContexts ?? [];
/** The legal contexts only the recall grids list (`lg-recall`): cells of their own, like a recall context of the misuse grid. */
export const recallLegalOf = (code) => SPEC[code].recallLegal ?? [];
export const bindingsOf = (code, variant) => SPEC[code].bindingsFor?.(variant) ?? SPEC[code].bindings;
export const variantsOf = (code) => SPEC[code].variants ?? [null];
/** The variants of a code's legal programs (legal.mjs): `legalVariants`, else the misuse variants. */
export const legalVariantsOf = (code) => SPEC[code].legalVariants ?? variantsOf(code);
/** The variants only the recall grids cross (`recallVariants`, `recallLegalVariants`), each on the base bindings and layers. */
export const recallVariantsOf = (code, { legal = false } = {}) => (legal ? SPEC[code].recallLegalVariants : SPEC[code].recallVariants) ?? [];
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
  return kind === "api" ? [...RECALL_API_BINDINGS, ...CALLEE_FORMS] : kind === "local" ? [...RECALL_VALUE_BINDINGS, ...HOOK_SLOT_BINDINGS, ...CALLEE_FORMS] : RECALL_VALUE_BINDINGS;
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
  const out = { text: "", solid: [], web: [], namespaces: [], moduleLines: [], preludeLines: [], contextLine: typeof spec.contextLine === "function" ? spec.contextLine(node.variant) : spec.contextLine };
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
      // ADR 0780: the namespace is an export of a module, with no `import * as` of the primitive's module anywhere.
      case "reexport-ns-as": out.importLines = ['import { Api as R } from "./solid-api";']; out.files = { "solid-api.ts": `export * as Api from "${module}";\n` }; out.text = wrap(`R.${name}`); break;
      case "reexport-ns": out.importLines = ['import * as R from "./solid-api";']; out.files = { "solid-api.ts": `export * from "${module}";\n` }; out.text = wrap(`R.${name}`); break;
      case "bind": importsOf(module).push(name); out.text = `${name}.bind(null)(${args})`; break;
      case "call": importsOf(module).push(name); out.text = `${name}.call${callTypeArguments(name, args)}(null${args ? `, ${args}` : ""})`; break;
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
    if (HOOK_SLOT_BINDINGS.includes(node.bind)) {
      const value = bare ? name : `${name}(${args})`;
      if (node.bind.endsWith("-written")) out.moduleLines.push("export const other = { useIt: () => 0 };", "other.useIt = () => 1;");
      const form = node.bind.replace(/-written$/, "");
      out.preludeLines.push({
        "hook-fn-slot": `const useThing = () => ({ useIt: () => { ${AUX}return ${value}; } });`,
        "hook-fn-member": `const useThing = () => ({ useIt: () => { ${AUX}return ${value}; } });`,
        "hook-method-slot": `const useThing = () => ({ useIt() { ${AUX}return ${value}; } });`,
        "hook-shorthand-fn": `const useThing = () => { const useIt = () => { ${AUX}return ${value}; }; return { useIt }; };`,
      }[form]);
      if (form === "hook-fn-member") { out.preludeLines.push("const thing = useThing();"); out.text = "thing.useIt()"; }
      else { out.preludeLines.push("const { useIt } = useThing();"); out.text = "useIt()"; }
      return out;
    }
    switch (node.bind) {
      case "alias": out.preludeLines.push(`const useIt = ${name};`); out.text = `useIt(${args})`; break;
      case "destructured": out.preludeLines.push(`const { useIt } = { useIt: ${name} };`); out.text = `useIt(${args})`; break;
      case "wrapper": out.preludeLines.push(`const wrapUse = () => { ${AUX}${bare ? name : `${name}(${args})`}; };`); out.text = "wrapUse()"; break;
      case "hook": out.preludeLines.push(`const useThing = () => ({ useIt: ${name} });`, "const { useIt } = useThing();"); out.text = `useIt(${args})`; break;
      case "hook-member": out.preludeLines.push(`const useThing = () => ({ useIt: ${name} });`, "const thing = useThing();"); out.text = `thing.useIt(${args})`; break;
      case "tuple": out.preludeLines.push(`const pair = [0, ${name}] as const;`, "const [, useIt] = pair;"); out.text = `useIt(${args})`; break;
      case "bind": out.text = `${name}.bind(null)(${args})`; break;
      case "call": out.text = `${name}.call${callTypeArguments(name, args)}(null${args ? `, ${args}` : ""})`; break;
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
  out.solid.push(...(target.imports ?? []));
  if (target.importLines) out.importLines = target.importLines;
  if (target.files) out.files = target.files;
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
  if (CONTROL_LAYERS.includes(layer) || HANDOFF_FORMS.includes(layer) || DEPTH_FORMS.includes(layer) || CHAIN_FORMS.includes(layer)) return [ctl(layer, stmts)];
  if (FILE_LAYERS.includes(layer)) return [expr(call(ref(kit.ensureHelper("call", { module: true, via: layer.slice("module-".length) })), [fn(stmts)]))];
  switch (layer) {
    case "closure": { const f = kit.name("f"); return [decl(f, fn(stmts)), expr(call(ref(f)))]; }
    case "helper": return [expr(call(ref(kit.ensureHelper("call")), [fn(stmts)]))];
    case "module-helper": return [expr(call(ref(kit.ensureHelper("call", { module: true })), [fn(stmts)]))];
    case "iife": return [expr(iife(fn(stmts)))];
    default: return stmts;
  }
}

/** The `<Loading on={t()}>` boundary's values: the trigger it holds on, and an async memo that reads it. */
function loadingSignals(name) {
  const trigger = signal(name("t"), 1, "src");
  return [trigger, asyncDepSignal(name("a"), trigger.name)];
}

/**
 * Places `stmts` (which hold the site) in a context. Returns { body, view } (the statements of the component body and its JSX items).
 * Contexts that a layer cannot wrap (an async compute needs its await first) take the layers inside.
 */
/** The element, the listener and the call of a fire (FIRES), with `handler` the listener's body. */
function fireParts(fire, handler, kit) {
  const el = kit.name("el");
  const base = fire.replace(/-(wrapper|member)$/, "");
  const made = (tag) => [decl(el, domExpr(`document.createElement("${tag}")`)), expr(domExpr(`document.body.append(${el})`))];
  const on = (event) => expr(listenExpr(el, event, fn(handler)));
  const focused = expr(domExpr(`${el}.focus()`));
  if (fire === "steal") {
    const other = kit.name("el");
    return { setup: [...made("input"), decl(other, domExpr('document.createElement("input")')), expr(domExpr(`document.body.append(${other})`)), on("blur"), focused], fire: expr(domExpr(`${other}.focus()`)) };
  }
  if (base === "focus") {
    if (fire === "focus-wrapper") { const go = kit.name("fire"); return { setup: [...made("input"), on("focus"), decl(go, fn([focused]))], fire: expr(call(ref(go))) }; }
    if (fire === "focus-member") { const box = kit.name("box"); return { setup: [...made("input"), on("focus"), decl(box, domExpr(`{ held: ${el} }`))], fire: expr(domExpr(`${box}.held.focus()`)) }; }
    return { setup: [...made("input"), on("focus")], fire: focused };
  }
  switch (fire) {
    case "select": return { setup: [...made("input"), on("focus")], fire: expr(domExpr(`${el}.select()`)) };
    case "blur": case "remove": return { setup: [...made("input"), on("blur"), focused], fire: expr(domExpr(`${el}.${fire}()`)) };
    case "reset": return { setup: [...made("form"), on("reset")], fire: expr(domExpr(`${el}.reset()`)) };
    case "submit": return { setup: [...made("form"), on("submit"), expr(domExpr(`${el}.addEventListener("submit", (event) => event.preventDefault())`))], fire: expr(domExpr(`${el}.requestSubmit()`)) };
    case "validity": return { setup: [...made("input"), expr(domExpr(`${el}.required = true`)), on("invalid")], fire: expr(domExpr(`${el}.checkValidity()`)) };
    // Closed on the spot: an open modal makes the document inert, and the driver's next click then waits out its timeout (ADR 0679's run).
    case "modal": return { setup: [...made("dialog"), on("beforetoggle")], fire: expr(domExpr(`(${el}.showModal(), ${el}.close())`)) };
    case "popover": return { setup: [...made("div"), expr(domExpr(`${el}.popover = "auto"`)), on("beforetoggle")], fire: expr(domExpr(`${el}.showPopover()`)) };
    default: throw new Error(`no fire ${fire}`);
  }
}

/** A fire at a position (`evt-<position>-fire-<fire>`): the stack the delivery runs on. */
function fireContext(position, fire, stmts, kit) {
  const v = () => kit.name("v");
  if (fire === "project") { const lens = kit.name("lens"); return { body: [decl(lens, domExpr("new (class { focus() { return 1; } })()")), expr(domExpr(`${lens}.focus()`))], view: [button(kit.name("b"), stmts)] }; }
  if (fire === "shadow") { const focus = "focus"; return { body: [decl(focus, fn([ret(num(1))])), expr(call(ref(focus)))], view: [button(kit.name("b"), stmts)] }; }
  const { setup, fire: call_ } = fireParts(fire, stmts, kit);
  switch (position) {
    case "body": return { body: [...setup, call_], view: [] };
    case "memo": { const m = kit.name("m"); return { body: [...setup, decl(m, prim("memo", fn([call_, ret(num(1))])))], view: [child(v(), call(ref(m)))] }; }
    case "compute": return { body: [...setup, expr(prim("effect", fn([call_, ret(num(1))]), { apply: fn([]) }))], view: [] };
    case "settled": return { body: [...setup, expr(prim("settled", fn([call_])))], view: [] };
    case "untrack": { const m = kit.name("m"); return { body: [...setup, decl(m, prim("memo", fn([expr(prim("untrack", fn([call_]))), ret(num(1))])))], view: [child(v(), call(ref(m)))] }; }
    case "apply": return { body: [...setup, expr(prim("effect", fn([ret(num(1))]), { apply: fn([call_]) }))], view: [] };
    case "handler": return { body: setup, view: [button(kit.name("b"), [call_])] };
    default: throw new Error(`no fire position ${position}`);
  }
}

/** A window listener (`lst-<position>-<form>`, ADR 0645): `dispatch` runs it, or nothing does. */
function listenerContext(position, form, stmts, kit) {
  const box = kit.name("box");
  const dispatch = 'window.dispatchEvent(new Event("click"))';
  const held = decl(box, obj([["handle", fn(stmts)]]));
  const button_ = () => button(kit.name("b"), position === "handler" ? [expr(domExpr(dispatch))] : []);
  const at = position === "body" ? [expr(domExpr(dispatch))] : [];
  const view = position === "handler" ? [button_()] : [];
  switch (form) {
    case "method": return { body: [held, expr(listenExpr("window", "click", fn([expr(call(memberOf(ref(box), "handle")))]))), ...at], view };
    // At module scope nothing placed the method: `const`, listener and signal outside the component that dispatches.
    case "module": return { hoisted: [held, expr(listenExpr("window", "click", fn([expr(call(memberOf(ref(box), "handle")))])))], body: at, view };
    case "bind": return { body: [held, expr(domExpr(`window.addEventListener("click", ${box}.handle.bind(${box}))`)), ...at], view };
    case "object": return { body: [held, expr(domExpr(`window.addEventListener("click", { handleEvent: ${box}.handle })`)), ...at], view };
    case "shared": { const h = kit.name("f"); return { body: [decl(h, fn(stmts)), expr(domExpr(`window.addEventListener("click", ${h})`)), ...at], view: [...view, namedButton(kit.name("b"), ref(h))] }; }
    default: throw new Error(`no listener form ${form}`);
  }
}

/**
 * `evt-apply-owned-flush` (`g10`): a later root's body flushes under its owner, the drained apply clicks, and the JSX handler has that owner.
 * ADR 0639's effect window takes only its event, so a read there is silent; the write throws.
 */
function ownedFlushContext(stmts, kit) {
  const go = apiSignal(kit.name("t"), 0, "owned", "src");
  const b = kit.name("b"), host = kit.name("el");
  return {
    sigs: [go],
    body: [
      decl(host, domExpr('document.createElement("div")')),
      expr(prim("effect", fn([ret(read(go.name))]), { apply: fn([expr(domExpr(`v && document.querySelector<HTMLElement>('[data-act="${b}"]')!.click()`))]) })),
      decl("Later", domExpr(`() => { ${go.setter}(1); flush(); return <i />; }`, undefined, ["flush"])),
      expr(prim("timer", fn([expr(domExpr(`render(() => <Later />, ${host})`, ["render"]))]))),
    ],
    view: [button(b, stmts)],
  };
}

const anyEl = (id) => `(document.querySelector('[data-act="${id}"]') as any)`;
function laterRoot(kit, prep, trigger, solid, web) {
  const host = kit.name("el");
  return [
    decl(host, domExpr('document.createElement("div")')),
    decl("Later", domExpr(`() => { ${trigger}; return <i />; }`, web, solid)),
    expr(prim("timer", fn([...prep.map((text) => expr(domExpr(text))), expr(domExpr(`render(() => <Later />, ${host})`, ["render"]))]))),
  ];
}
/**
 * The contexts whose own timer disables or removes the driven element before any click: nothing can click it, the fast hosts skip it, and
 * Playwright waits out its actionability check (12 of 29 sampled `dny-*` programs were `harness-error` in Chrome, every time, so these
 * contexts had no oracle at all). `undriven` takes it off the click plan; the trigger, not the driver, is what these contexts measure.
 */
const TAKEN_AWAY = new Set(["dny-blur-disabled", "dny-blur-replace", "dny-focusout-replace", "dny-change-typed", "dny-blur-select-remove",
  ...Object.keys(DISPOSALS), "dny-unmount-flush-body", "dny-unmount-flush-root", "dny-insert-body", "dny-shared-fn", "dny-chain-click",
  "dny-flush-unmount", "dny-blur-spread", "dny-blur-dynamic", "dny-portal-dispose", "dny-blur-lowercase", "dny-legal-keydown", "dny-legal-click", "dny-legal-pointer"]);
function undriven(tree, id) {
  if (Array.isArray(tree)) { for (const item of tree) undriven(item, id); return; }
  if (tree === null || typeof tree !== "object") return;
  if ((tree.k === "button" || tree.k === "hbutton") && tree.id === id) tree.undriven = true;
  for (const value of Object.values(tree)) undriven(value, id);
}
function deniedContext(context, stmts, kit) {
  const b = kit.name("b");
  const built = deniedContextOf(context, stmts, kit, b);
  if (TAKEN_AWAY.has(context)) undriven(built, b);
  return built;
}
function deniedContextOf(context, stmts, kit, b) {
  const el = anyEl(b);
  if (context === "dny-blur-lowercase") { const h = kit.name("f"); return { body: [decl(h, fn(stmts)), ...laterRoot(kit, [`${el}.focus()`], `${el}.disabled = true`)], view: [button(b, [], { on: "onKeyDown", attrs: `{...{ onblur: ${h} }}` })] }; }
  if (context === "dny-legal-project-click") return { body: laterRoot(kit, [], "({ click() { return 1; } }).click()"), view: [button(b, stmts)] };
  const legal = { "dny-legal-keydown": { tag: "input", on: "onKeyDown" }, "dny-legal-click": { tag: "button", on: "onClick" }, "dny-legal-pointer": { tag: "button", on: "onPointerDown" } }[context];
  if (legal) return { body: laterRoot(kit, [`${el}.focus()`], `${el}.disabled = true`), view: [button(b, stmts, legal)] };
  const spec = DENIED[context];
  if (spec) {
    const fill = (text) => text.replaceAll("EL", el);
    const declared = spec.declares ? [expr(domExpr('declare module "@solidjs/web" { namespace JSX { interface IntrinsicElements { "x-sc": any } } }')),
      expr(domExpr('if (!customElements.get("x-sc")) customElements.define("x-sc", class extends HTMLElement { static formAssociated = true; internals = this.attachInternals(); })'))] : [];
    return { ...(declared.length ? { hoisted: declared } : {}), body: laterRoot(kit, spec.prep.map(fill), fill(spec.trigger)), view: [button(b, stmts, { tag: spec.tag, on: spec.on, attrs: spec.attrs })] };
  }
  if (DISPOSALS[context]) {
    const host = kit.name("el"), d = kit.name("f");
    const dispose = { body: `${d}()`, memo: `createMemo(() => { ${d}(); return 1; })()`, compute: `createEffect(() => { ${d}(); return 1; }, () => {})`, untrack: `createMemo(() => untrack(() => { ${d}(); return 1; }))()` }[DISPOSALS[context]];
    const inner = decl("Inner", fn([ret(jsxExpr(button(b, stmts, { tag: "input", on: "onBlur" })))]));
    return { body: [decl(host, domExpr('document.createElement("div")')), expr(domExpr(`document.body.append(${host})`)), inner,
      decl(d, domExpr(`render(() => <Inner />, ${host})`, ["render"])),
      ...laterRoot(kit, [`${el}.focus()`], dispose, ["createEffect", "createMemo", "untrack"])], view: [] };
  }
  const gone = apiSignal(kit.name("t"), context === "dny-flush-load" ? 0 : 1, "owned", "src");
  switch (context) {
    case "dny-unmount-flush-body": case "dny-unmount-flush-root": {
      const change = context.endsWith("root") ? `createRoot(() => { ${gone.setter}(0); flush(); })` : `${gone.setter}(0); flush()`;
      return { sigs: [gone], body: laterRoot(kit, [`${el}.focus()`], change, ["createRoot", "flush"]), view: [show(read(gone.name), [button(b, stmts, { tag: "input", on: "onBlur" })])] };
    }
    case "dny-insert-body": {
      const host = kit.name("el");
      return { body: [decl(host, domExpr('document.createElement("div")')), expr(domExpr(`document.body.append(${host})`)), decl("Inner", fn([ret(jsxExpr(button(b, stmts, { tag: "input", on: "onBlur" })))])), expr(domExpr(`render(() => <Inner />, ${host})`, ["render"])), ...laterRoot(kit, [`${el}.focus()`], `insert(${host}, "x")`, undefined, ["insert"])], view: [] };
    }
    case "dny-renamed-prop": {
      const field = decl("Field", domExpr(`(props: { save: () => void }) => <button data-act="${b}" onBlur={props.save}>go</button>`));
      return { body: [field, ...laterRoot(kit, [`${el}.focus()`], `${el}.disabled = true`)], view: [propEl("Field", fn(stmts), "save")] };
    }
    case "dny-shared-fn": { const h = kit.name("f"); return { body: [decl(h, fn(stmts)), ...laterRoot(kit, [`${el}.focus()`], `${el}.disabled = true`)], view: [namedButton(b, ref(h), { attrs: `onBlur={${h}}` })] }; }
    case "dny-chain-click": {
      const u = kit.name("b");
      return { body: laterRoot(kit, [`${el}.focus()`], `${el}.disabled = true`), view: [button(u, stmts), button(b, [expr(domExpr(`document.querySelector<HTMLElement>('[data-act="${u}"]')!.click()`))], { on: "onBlur" })] };
    }
    case "dny-flush-unmount": return { sigs: [gone], body: laterRoot(kit, [`${el}.focus()`, `${gone.setter}(0)`], "1"), view: [show(read(gone.name), [button(b, stmts, { tag: "input", on: "onBlur" })])] };
    case "dny-blur-spread": { const hs = kit.name("box"); return { body: [decl(hs, obj([["onBlur", fn(stmts)]])), ...laterRoot(kit, [`${el}.focus()`], `${el}.disabled = true`)], view: [button(b, [], { on: "onKeyDown", attrs: `{...${hs}}` })] }; }
    // An attribute `onblur` and a typo fail tsc (invariant 1), and rc.21 has no `on:` (the types reject it, the compiler refuses the source).
    // A spread takes either name. rc.21 binds `onBlurr` and a dispatch delivers it, so it falls to the closed default; it does not bind
    // `onblur` (LOWERCASE_EVENT_ATTRIBUTE), so that site never runs.
    case "dny-typo-event": { const h = kit.name("f"); return { body: [decl(h, fn(stmts)), ...laterRoot(kit, [], `${el}.dispatchEvent(new Event("blurr"))`)], view: [button(b, [], { on: "onKeyDown", attrs: `{...{ onBlurr: ${h} }}` })] }; }
    case "dny-blur-dynamic": return { body: laterRoot(kit, [`${el}.focus()`], `${el}.disabled = true`, undefined, ["Dynamic"]), view: [button(b, stmts, { tag: "Dynamic", on: "onBlur", attrs: 'component="button"' })] };
    case "dny-portal-dispose": {
      const d = kit.name("f"), out = kit.name("el");
      return { body: [decl("Inner", fn([ret(jsxExpr(button(b, stmts, { tag: "input", on: "onBlur" })))])), decl(d, domExpr("(() => {}) as () => void"), { kw: "let" }),
        decl(out, domExpr('document.createElement("div")')), expr(domExpr(`document.body.append(${out})`)),
        expr(domExpr(`queueMicrotask(() => { createRoot((dd) => { ${d} = dd; insert(${out}, () => <Portal><Inner /></Portal>); }); })`, ["insert", "Portal"], ["createRoot"])),
        ...laterRoot(kit, [`${el}.focus()`], `${d}()`)], view: [] };
    }
    // ADR 0780: inline in a control-flow callback, a nested handler keeps the denied arm.
    case "dny-load-for": case "dny-load-show": {
      const frame = kit.name("r");
      return { body: [], view: [flowEl(kit.name("v"), context.slice(9), [decl(frame, jsxExpr(button(b, stmts, { tag: "iframe", on: "onLoad" }))), expr(domExpr(`document.body.append(${frame} as Node)`))])] };
    }
    case "dny-load-append": return { body: [decl("Inner", fn([ret(jsxExpr(button(b, stmts, { tag: "iframe", on: "onLoad" })))])), ...laterRoot(kit, [], "document.body.appendChild(Inner() as any)")], view: [] };
    case "dny-flush-popover": return { sigs: [gone], body: laterRoot(kit, [`${el}.showPopover()`, `${gone.setter}(0)`], "1"), view: [button(b, stmts, { tag: "div", on: "onBeforeToggle", attrs: `popover={${gone.name}() ? "auto" : undefined}` })] };
    case "dny-flush-load": return { sigs: [gone], body: [expr(prim("timer", fn([expr(domExpr(`${gone.setter}(1)`))])))], view: [show(read(gone.name), [button(b, stmts, { tag: "iframe", on: "onLoad" })])] };
    default: throw new Error(`no denied context ${context}`);
  }
}

/** An apply a nested `render` drains (`rnd-*`, ADR 0644): the site is in the apply, queued before the mount runs. */
function renderContext(where, stmts, kit) {
  const mount = domExpr('render(() => null, document.createElement("div"))', ["render"]);
  const apply = expr(prim("effect", fn([ret(num(1))]), { apply: fn(stmts) }));
  const v = () => kit.name("v");
  switch (where) {
    case "body": return { body: [apply, expr(mount)], view: [] };
    case "wrapper": { const f = kit.name("f"); return { body: [apply, decl(f, fn([expr(mount)])), expr(call(ref(f)))], view: [] }; }
    case "root": return { body: [apply, expr(prim("root", fn([expr(mount)])))], view: [] };
    case "memo": { const m = kit.name("m"); return { body: [apply, decl(m, prim("memo", fn([expr(mount), ret(num(1))])))], view: [child(v(), call(ref(m)))] }; }
    case "foreach": return { body: [apply, expr(domExpr('[document.createElement("div")].forEach((el) => render(() => null, el))', ["render"]))], view: [] };
    case "handler": return { body: [apply], view: [button(kit.name("b"), [expr(mount)])] };
    case "timer": return { body: [apply, expr(prim("timer", fn([expr(mount)])))], view: [] };
    case "await": { const f = kit.name("f"); return { body: [apply, decl(f, fn([awaitStmt(), expr(mount)], { async: true })), expr(call(ref(f)))], view: [] }; }
    case "shadow": { const f = "render"; return { body: [apply, decl(f, fn([ret(num(1))])), expr(call(ref(f)))], view: [] }; }
    default: throw new Error(`no render position ${where}`);
  }
}

/** The elements and registrations of ADR 0645's anchor census: each delivers `stmts` on the stack of the apply that clicks. */
function anchorContext(context, stmts, kit) {
  const fire = kit.name("fire"), el = kit.name("el"), h = kit.name("f");
  const clicks = (tag, ...set) => decl(fire, fn([decl(el, domExpr(`document.createElement("${tag}")`)), ...set.map((text) => expr(domExpr(text.replaceAll("EL", el)))), expr(domExpr(`document.body.append(${el})`)), expr(domExpr(`${el}.click()`)), expr(domExpr(`${el}.remove()`))]));
  const apply = expr(prim("effect", fn([ret(num(1))]), { apply: fn([expr(call(ref(fire)))]) }));
  const listener = decl(h, fn(stmts));
  switch (context) {
    case "evt-anchor-apply-bare": return { body: [listener, expr(domExpr(`addEventListener("click", ${h})`)), clicks("a"), apply], view: [] };
    case "evt-anchor-apply-alias": { const l = kit.name("box"); return { body: [listener, decl(l, domExpr("window.addEventListener")), expr(domExpr(`(${l} as any).call(window, "click", ${h})`)), clicks("a"), apply], view: [] }; }
    case "evt-anchor-apply-computed": return { body: [listener, expr(domExpr(`window["addEventListener"]("click", ${h})`)), clicks("a"), apply], view: [] };
    case "evt-created-checkbox": return { body: [listener, expr(domExpr(`document.addEventListener("change", ${h})`)), clicks("input", 'EL.type = "checkbox"'), apply], view: [] };
    case "evt-created-label": { const id = kit.name("b"); return { body: [clicks("label", 'EL.htmlFor = "sc-labelled"', `document.querySelector('[data-act="${id}"]')!.id = "sc-labelled"`), apply], view: [button(id, stmts)] }; }
    default: throw new Error(`no anchor context ${context}`);
  }
}

/** `zero-<where>-<form>` (ADRs 0563, 0566, 0580): a site in a carrier that runs it zero times, on a stack that would otherwise be a misuse. */
const ZERO_RUNS = ["zero-body-empty", "zero-body-var", "zero-settled-empty", "zero-settled-var", "zero-await-empty", "zero-await-var"];
function zeroRunContext(where, form, stmts, kit) {
  const held = ctl(`handoff-foreach-${form}`, stmts);
  switch (where) {
    case "body": return { body: [held], view: [] };
    case "settled": return { body: [expr(prim("settled", fn([held])))], view: [] };
    case "await": { const f = kit.name("f"); return { body: [decl(f, fn([awaitStmt(), held], { async: true })), expr(call(ref(f)))], view: [] }; }
    default: throw new Error(`no zero-run position ${where}`);
  }
}

function inContext(context, stmts, kit) {
  if (context === "evt-apply-owned-flush") return ownedFlushContext(stmts, kit);
  const flipped = flipReadContext(context, stmts, kit) ?? flipWriteContext(context, stmts, kit);
  if (flipped) return flipped;
  if (context.startsWith("dny-")) return deniedContext(context, stmts, kit);
  const zero = /^zero-(body|settled|await)-(empty|var)$/.exec(context);
  if (zero) return zeroRunContext(zero[1], zero[2], stmts, kit);
  // ADR 0641: a component's prop is a getter nothing reads, whatever spells the tag: a capital, a member, a lowercase member.
  const tagged = { "prop-unread-cap": "Sink", "prop-unread-member": "Sinks.Sink", "prop-unread-lower": "sinks.sink" }[context];
  if (tagged) return { body: [decl("Sink", domExpr("(_props: { a: unknown }) => <i />")), decl("Sinks", domExpr("{ Sink }")), decl("sinks", domExpr("{ sink: Sink }"))], view: [propEl(tagged, stmts[0].e)] };
  // ADR 0542: a component handed to plain code as a function: whether its body is a component's is not proven.
  if (context === "handed-flush") return { body: [decl("Row", fn([expr(flush(fn(stmts))), ret(jsx())]))], view: [child(kit.name("v"), iife(fn([expr(domExpr("[1].map(Row)")), ret(num(1))])))] };
  const listener = /^lst-(body|none|handler)-(.+)$/.exec(context);
  if (listener) return listenerContext(listener[1], listener[2], stmts, kit);
  if (context.startsWith("rnd-")) return renderContext(context.slice(4), stmts, kit);
  if (ANCHOR_MISSES.includes(context)) return anchorContext(context, stmts, kit);
  const fired = /^evt-(body|memo|compute|settled|untrack|apply|handler)-fire-(.+)$/.exec(context);
  if (fired) return fireContext(fired[1], fired[2], stmts, kit);
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
    // ADR 0641: the guard open, so the site a `-guarded` row never runs is judged once it runs.
    case "effect-apply-guard-open": return { body: [expr(prim("effect", fn([ret(num(2))]), { apply: fn([contain("guard-return", 0, stmts)]) }))], view: [] };
    case "render-apply-guarded": return { body: [expr(prim("render", fn([ret(num(1))]), { apply: fn([contain("guard-return", 0, stmts)]) }))], view: [] };
    case "tracked": return { body: [expr(prim("tracked", fn(stmts)))], view: [] };
    case "settled": return { body: [expr(prim("settled", fn(stmts)))], view: [] };
    case "untrack": return { body: [expr(prim("untrack", fn(stmts)))], view: [] };
    case "root": return { body: [expr(prim("root", fn(stmts)))], view: [] };
    case "owner-null": return { body: [expr(prim("owner", fn(stmts), { ownerArg: "null" }))], view: [] };
    case "timer": return { body: [expr(prim("timer", fn(stmts)))], view: [] };
    case "then": return { body: [expr(prim("then", fn(stmts)))], view: [] };
    case "micro": return { body: [expr(prim("micro", fn(stmts)))], view: [] };
    case "handler": case "creates-in-handler": return { body: [], view: [button(kit.name("b"), stmts)] };
    // EVENT_CONTEXTS: a listener the program dispatches to itself, and work a handler or an apply schedules.
    case "evt-body-dispatch": case "evt-apply-dispatch": case "evt-handler-dispatch": {
      const el = kit.name("el");
      const made = [decl(el, domExpr('document.createElement("button")')), expr(listenExpr(el, "click", fn(stmts)))];
      const fire = expr(domExpr(`${el}.click()`));
      if (context === "evt-body-dispatch") return { body: [...made, fire], view: [] };
      if (context === "evt-apply-dispatch") return { body: [...made, expr(prim("effect", fn([ret(num(1))]), { apply: fn([fire]) }))], view: [] };
      return { body: made, view: [button(kit.name("b"), [fire])] };
    }
    // ADR 0639: `flush()`'s guard takes no event, and the dispatch that reaches it may sit in a helper or on `window`.
    case "evt-apply-helper-dispatch": {
      const el = kit.name("el"), fire = kit.name("fire");
      const helper = decl(fire, fn([expr(domExpr(`${el}.click()`))]));
      return { body: [decl(el, domExpr('document.createElement("button")')), expr(listenExpr(el, "click", fn(stmts))), helper, expr(prim("effect", fn([ret(num(1))]), { apply: fn([expr(call(ref(fire)))]) }))], view: [] };
    }
    case "evt-apply-window-dispatch":
      return { body: [expr(listenExpr("window", "sc-evt", fn(stmts))), expr(prim("effect", fn([ret(num(1))]), { apply: fn([expr(domExpr('window.dispatchEvent(new Event("sc-evt"))'))]) }))], view: [] };
    // ADR 0640: an anchor a helper creates and clicks under `document.body` delivers to no listener, so another handler keeps its proof;
    // with a `click` listener on `window` it delivers there, on the stack of the apply that clicks.
    case "evt-created-anchor": case "evt-anchor-apply-listener": {
      const anchor = kit.name("el"), fire = kit.name("fire");
      const helper = decl(fire, fn([decl(anchor, domExpr('document.createElement("a")')), expr(domExpr(`document.body.append(${anchor})`)), expr(domExpr(`${anchor}.click()`)), expr(domExpr(`${anchor}.remove()`))]));
      if (context === "evt-created-anchor") return { body: [helper, expr(call(ref(fire)))], view: [button(kit.name("b"), stmts)] };
      return { body: [helper, expr(listenExpr("window", "click", fn(stmts))), expr(prim("effect", fn([ret(num(1))]), { apply: fn([expr(call(ref(fire)))]) }))], view: [] };
    }
    case "timer-from-handler": return { body: [], view: [button(kit.name("b"), [expr(prim("timer", fn(stmts)))])] };
    case "micro-from-handler": return { body: [], view: [button(kit.name("b"), [expr(prim("micro", fn(stmts)))])] };
    case "timer-from-apply": return { body: [expr(prim("effect", fn([ret(num(1))]), { apply: fn([expr(prim("timer", fn(stmts)))]) }))], view: [] };
    case "micro-from-apply": return { body: [expr(prim("effect", fn([ret(num(1))]), { apply: fn([expr(prim("micro", fn(stmts)))]) }))], view: [] };
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
  // ADR 0613: the handler that makes a rerun and drains it under a root.
  const drainRerun = (t) => [expr(write(t.name, 2)), expr(prim("root", fn([expr(flush())])))];
  const after = /^(memo-async|effect-async|async|handler-async)-(loop|for-await|finally)$/.exec(context);
  if (after) {
    const body = [ctl(awaited[after[2]], stmts)];
    // A handler's async function: the site may follow the await (a `finally`, a loop's second pass), so it runs on the handler's
    // stack or on a later task, with no owner either way (ADR 0610).
    if (after[1] === "handler-async") {
      const f = kit.name("f");
      return { body: [decl(f, fn(body, { async: true }))], view: [button(kit.name("b"), [expr(call(ref(f)))])] };
    }
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
    case "render-cleanup-rerun": {
      const t = trigger();
      return { body: [expr(prim("render", fn([ret(read(t.name))]), { apply: fn([ret(fn(stmts))]) }))], view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t] };
    }
    // ADR 0539: the apply named by a binding, not written inline.
    case "effect-cleanup-rerun-named": {
      const t = trigger();
      const a = kit.name("apply");
      return { body: [decl(a, fn([ret(fn(stmts))])), expr(prim("effect", fn([ret(read(t.name))]), { apply: ref(a) }))], view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t] };
    }
    case "effect-cleanup-dispose": {
      const d = kit.name("disp");
      const effect = expr(prim("effect", fn([ret(num(1))]), { apply: fn([ret(fn(stmts))]) }));
      return { body: [decl(d, prim("rootd", fn([effect, ret(ref("dispose"))])))], view: [button(kit.name("b"), [expr(call(ref(d)))])] };
    }
    case "effect-cleanup-show": case "effect-cleanup-for": {
      const t = trigger();
      const effect = expr(prim("effect", fn([ret(num(1))]), { apply: fn([ret(fn(stmts))]) }));
      const flow = context === "effect-cleanup-show" ? "show" : "for";
      return { body: [], view: [flowEl(kit.name("v"), flow, [effect], { on: t.name }), button(kit.name("b"), [expr(write(t.name, 0))])], sigs: [t] };
    }
    case "effect-error-arm": {
      const t = trigger();
      return { body: [expr(prim("effect", fn(stmts), { errorArm: t.name }))], view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t] };
    }
    case "reaction": {
      const t = trigger();
      const track = kit.name("track");
      return { body: [decl(track, prim("reaction", fn(stmts))), expr(call(ref(track), [fn([expr(read(t.name))])]))], view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t] };
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
    // ACTION_STEP_CONTEXTS: the site in an action's first synchronous slice, the action called by the named caller.
    case "action-from-apply": case "action-plain-yield-from-apply": {
      const a = kit.name("a");
      const act = prim("action", fn(stmts), context === "action-plain-yield-from-apply" ? { plainYield: true } : {});
      return { body: [decl(a, act), expr(prim("effect", fn([ret(num(1))]), { apply: fn([expr(call(ref(a)))]) }))], view: [] };
    }
    case "action-from-reaction": {
      const t = trigger();
      const a = kit.name("a");
      const track = kit.name("track");
      return {
        body: [decl(a, prim("action", fn(stmts))), decl(track, prim("reaction", fn([expr(call(ref(a)))]))), expr(call(ref(track), [fn([expr(read(t.name))])]))],
        view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t],
      };
    }
    case "action-from-settled": case "action-from-tracked": {
      const a = kit.name("a");
      return { body: [decl(a, prim("action", fn(stmts))), expr(prim(context === "action-from-settled" ? "settled" : "tracked", fn([expr(call(ref(a)))])))], view: [] };
    }
    case "action-from-error-arm": {
      const t = trigger();
      const a = kit.name("a");
      return { body: [decl(a, prim("action", fn(stmts))), expr(prim("effect", fn([expr(call(ref(a)))]), { errorArm: t.name }))], view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t] };
    }
    // ADR 0551: the site in `resolve`'s or `until`'s thunk, called in the body: the compute of a computed under a fresh root.
    case "resolve-thunk": return { body: [expr(prim("resolve", fn([...stmts, ret(num(1))])))], view: [] };
    case "until-thunk": return { body: [expr(prim("until", fn([...stmts, ret(num(1))])))], view: [] };
    // The site in a reaction bundle's `error` member: the tracker throws once the button sets 2.
    case "reaction-error-arm": {
      const t = trigger();
      const track = kit.name("track");
      return {
        body: [decl(track, prim("reaction", fn(stmts), { errorArm: true })), expr(call(ref(track), [{ ...fn([expr(read(t.name))]), throwOn: t.name }]))],
        view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t],
      };
    }
    // API_CONTEXTS: a lazy memo computes on its first read (`lazy: true`): never read, the site never runs; read in a handler, its
    // compute runs there, on the handler's stack, as the memo's own computation. A deferred apply (`defer: true`) skips the first run:
    // with no rerun the site never runs; `-rerun` reruns the compute from a button, and the apply then runs.
    case "memo-lazy-unread": { const m = kit.name("m"); return { body: [decl(m, { ...prim("memo", fn([...stmts, ret(num(1))])), lazy: true })], view: [] }; }
    case "memo-lazy-read-handler": { const m = kit.name("m"); return { body: [decl(m, { ...prim("memo", fn([...stmts, ret(num(1))])), lazy: true })], view: [button(kit.name("b"), [expr(call(ref(m)))])] }; }
    case "effect-apply-defer": return { body: [expr({ ...prim("effect", fn([ret(num(1))]), { apply: fn(stmts) }), defer: true })], view: [] };
    case "effect-apply-defer-rerun": {
      const t = trigger();
      return { body: [expr({ ...prim("effect", fn([ret(read(t.name))]), { apply: fn(stmts) }), defer: true })], view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t] };
    }
    // FLUSH_DRAIN_CONTEXTS (ADR 0613): the apply holds the site; the flush after its effect drains it on the flush's stack.
    case "effect-apply-flush-body": return { body: [expr(prim("effect", fn([ret(num(1))]), { apply: fn(stmts) })), expr(flush())], view: [] };
    case "effect-apply-flush-memo": {
      const m = kit.name("m");
      return { body: [expr(prim("effect", fn([ret(num(1))]), { apply: fn(stmts) })), decl(m, prim("memo", fn([expr(flush()), ret(num(1))])))], view: [child(kit.name("v"), call(ref(m)))] };
    }
    case "effect-apply-flush-root": return { body: [expr(prim("root", fn([expr(prim("effect", fn([ret(num(1))]), { apply: fn(stmts) })), expr(flush())])))], view: [] };
    // A handler's `onSettled` queues its callback (no owner there), and a root's `flush()` drains it under the root (ADR 0613).
    case "settled-flush-root": return { body: [], view: [button(kit.name("b"), [expr(prim("settled", fn(stmts))), expr(prim("root", fn([expr(flush())])))])] };
    case "settled-handler": return { body: [], view: [button(kit.name("b"), [expr(prim("settled", fn(stmts)))])] };
    case "effect-apply-flush-handler": return { body: [expr(prim("effect", fn([ret(num(1))]), { apply: fn(stmts) }))], view: [button(kit.name("b"), [expr(flush())])] };
    case "effect-cleanup-flush-rerun": case "render-cleanup-flush-rerun": {
      const t = trigger();
      return { body: [expr(prim(context.startsWith("effect") ? "effect" : "render", fn([ret(read(t.name))]), { apply: fn([ret(fn(stmts))]) }))], view: [button(kit.name("b"), drainRerun(t))], sigs: [t] };
    }
    case "render-apply-flush-rerun": {
      const t = trigger();
      return { body: [expr(prim("render", fn([ret(read(t.name))]), { apply: fn([contain("guard-return", 0, stmts)]) }))], view: [button(kit.name("b"), drainRerun(t))], sigs: [t] };
    }
    case "effect-error-arm-flush-rerun": {
      const t = trigger();
      return { body: [expr(prim("effect", fn(stmts), { errorArm: t.name }))], view: [button(kit.name("b"), drainRerun(t))], sigs: [t] };
    }
    case "reaction-flush-rerun": {
      const t = trigger();
      const track = kit.name("track");
      return { body: [decl(track, prim("reaction", fn(stmts))), expr(call(ref(track), [fn([expr(read(t.name))])]))], view: [button(kit.name("b"), drainRerun(t))], sigs: [t] };
    }
    case "effect-apply-defer-flush-rerun": {
      const t = trigger();
      return { body: [expr({ ...prim("effect", fn([ret(read(t.name))]), { apply: fn(stmts) }), defer: true })], view: [button(kit.name("b"), drainRerun(t))], sigs: [t] };
    }
    // FIRE_CONTEXTS: the site's function is written in the body and run elsewhere, later: handed to a project helper that defers it
    // (a timer, a microtask, a promise callback), kept in the object a helper returns and fired from a handler, the body or a timer,
    // or returned by a helper and called from a handler or the body.
    case "fire-helper-timer": return { body: [expr(call(ref(kit.ensureHelper("timerCb")), [fn(stmts)]))], view: [] };
    case "fire-helper-micro": return { body: [expr(call(ref(kit.ensureHelper("microCb")), [fn(stmts)]))], view: [] };
    case "fire-helper-then": return { body: [expr(call(ref(kit.ensureHelper("thenCb")), [fn(stmts)]))], view: [] };
    case "fire-store-handler": case "fire-store-body": case "fire-store-timer": {
      const box = kit.name("box");
      const fire = expr(call(memberOf(ref(box), "cb")));
      const kept = decl(box, call(ref(kit.ensureHelper("storeCb")), [fn(stmts)]));
      if (context === "fire-store-handler") return { body: [kept], view: [button(kit.name("b"), [fire])] };
      if (context === "fire-store-body") return { body: [kept, fire], view: [] };
      return { body: [kept, expr(prim("timer", fn([fire])))], view: [] };
    }
    case "fire-return-handler": case "fire-return-body": {
      const run = kit.name("run");
      const kept = decl(run, call(ref(kit.ensureHelper("returnCb")), [fn(stmts)]));
      return context === "fire-return-body" ? { body: [kept, expr(call(ref(run)))], view: [] } : { body: [kept], view: [button(kit.name("b"), [expr(call(ref(run)))])] };
    }
    // HANDLER_CONTEXTS: a handler passed by name (a `const`, a `function`, Solid's `[h, 1]` bound form), and one the body also calls.
    case "handler-named-const": case "handler-named-function": case "handler-tuple": case "handler-mixed": {
      // `hd`, not `h`: the same-file helpers are `hN` (generate.mjs ensureHelper), and a handler of that name would shadow one.
      const h = kit.name("hd");
      const handler = decl(h, context === "handler-named-function" ? fn(stmts, { form: "function" }) : fn(stmts));
      return { body: context === "handler-mixed" ? [handler, expr(call(ref(h)))] : [handler], view: [namedButton(kit.name("b"), ref(h), { tuple: context === "handler-tuple" })] };
    }
    // DEAD_CONTEXTS: code rc.17 never runs (model.mjs CONTAINER_KINDS): under `if (false)`, after a `return`, after `false &&`, in a
    // component never rendered. Legal for every code: a violation there is an unexecuted one (ADR 0458).
    case "dead-if": return { body: [contain("dead-if", 9, stmts)], view: [] };
    case "dead-after-return": return { body: [contain("dead-after-return", 9, stmts)], view: [] };
    // The block form (`false && (() => { …; return 1; })()`): a lone assignment after `false &&` would not parse.
    case "dead-and-false": return { body: [contain("and-false", 9, [...stmts, ret(num(1))])], view: [] };
    case "dead-comp-never": return { body: [contain("comp-never", 9, [...stmts, ret(jsx())])], view: [] };
    case "tracked-rerun": {
      const t = trigger();
      return { body: [expr(prim("tracked", fn([expr(read(t.name)), ...stmts])))], view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t] };
    }
    case "effect-apply-rerun": {
      const t = trigger();
      return { body: [expr(prim("effect", fn([ret(read(t.name))]), { apply: fn(stmts) }))], view: [button(kit.name("b"), [expr(write(t.name, 2))])], sigs: [t] };
    }
    case "ref": case "ref-array": case "ref-cond": return { body: [], view: [refEl(kit.name("v"), fn(stmts), context === "ref" ? "inline" : context.slice("ref-".length))] };
    case "ref-named": { const r = kit.name("rf"); return { body: [decl(r, fn(stmts))], view: [refEl(kit.name("v"), ref(r))] }; }
    case "ref-factory": { const d = kit.name("dir"); return { body: [decl(d, fn([ret(fn(stmts))]))], view: [refEl(kit.name("v"), call(ref(d)))] }; }
    case "ref-factory-body": { const d = kit.name("dir"); return { body: [decl(d, fn([...stmts, ret(fn([]))]))], view: [refEl(kit.name("v"), call(ref(d)))] }; }
    case "ref-row": return { body: [], view: [forEach([refEl(kit.name("v"), fn(stmts))])] };
    case "pos-attr": return { body: [], view: [attr(kit.name("v"), iife(fn([...stmts, ret(num(1))])))] };
    case "pos-class-sblock": return { body: [contain("class-sblock", 9, stmts)], view: [] };
    case "pos-class-field": return { body: [contain("class-field", 9, stmts, { use: "new" })], view: [] };
    case "pos-class-ctor": return { body: [contain("class-ctor", 9, stmts, { use: "new" })], view: [] };
    case "pos-class-method": return { body: [contain("class-method", 9, stmts, { use: "call" })], view: [] };
    case "pos-foreach": return { body: [expr(iter("forEach", fn(stmts)))], view: [] };
    case "pos-handler-expr": return { body: [], view: [handlerExpr(kit.name("v"), iife(fn([...stmts, ret(num(1))])))] };
    case "pos-flush-fn": return { body: [expr(flush(fn(stmts)))], view: [] };
    case "pos-handler-root": return { body: [], view: [button(kit.name("b"), [expr(prim("root", fn(stmts)))])] };
    case "pos-handler-untrack": return { body: [], view: [button(kit.name("b"), [expr(prim("untrack", fn(stmts)))])] };
    case "pos-handler-owner": return { body: [decl("boundOwner", createOwnerExpr())], view: [button(kit.name("b"), [expr(prim("owner", fn(stmts), { ownerArg: "bound" }))])] };
    // LOADING_ON_OUTSIDE_HOLD: the async memo (setup) reads the trigger; `<Loading on={t()}>` holds a read of it, the site reads it
    // outside, and a button writes the trigger, which re-arms the boundary while the outside read holds the frame.
    case "loading-on-outside": {
      const item = child(kit.name("v"), iife(fn([...stmts, ret(num(1))])));
      return { body: [], view: [item], loading: true };
    }
    // LOADING_ON_ACCESSOR: the boundary's `on`, as the variant writes it, around the site.
    case "loading-on-accessor": return { body: [], view: [child(kit.name("v"), iife(fn([...stmts, ret(num(1))])))], loadingOn: true };
    default: {
      // `cbp-<call>[-<form>]` (CBP_CONTEXTS): the site in a callback prop a same-file child calls.
      const prop = /^cbp-([a-z]+)(?:-([a-z]+))?$/.exec(context);
      if (prop && CBP_CALLS.includes(prop[1]) && CBP_FORMS.includes(prop[2] ?? "inline") && prop[2] !== "inline") {
        const [, call, form = "inline"] = prop;
        if (form !== "named") return { body: [], view: [cbProp(kit.name("v"), fn(stmts), call, form)] };
        const r = kit.name("run");
        return { body: [decl(r, fn(stmts, { form: "function" }))], view: [cbProp(kit.name("v"), ref(r), call, form)] };
      }
      // `flow-<form>` (FLOW_CONTEXTS): the site in the body of a flow component's function child.
      const flow = /^flow-(.+)$/.exec(context)?.[1];
      if (flow && FLOW_FORMS.includes(flow)) return { body: [], view: [flowEl(kit.name("v"), flow, stmts)] };
      throw new Error(`unknown misuse context ${context}`);
    }
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
  const recall = plan.legal ? recallLegalOf(code) : recallContextsOf(code);
  // A legal pair (matrix.mjs pairShapes, a later mount of a settled async source) sits in a misuse context of its code.
  const legalPair = plan.legal && plan.source?.startsWith("async-") && plan.mount !== undefined && spec.contexts.includes(context);
  // `plan.census` (matrix.mjs censusCells): any context, the measure being what the code does there.
  if (!contexts.includes(context) && !recall.includes(context) && !legalPair && !plan.census) throw new Error(`${code} has no ${plan.legal ? "legal " : ""}${context} context`);
  // A variant one source carries (`variantSources`) is the grid's, never drawn: every random draw sees the variants it saw before.
  const variant = "variant" in plan && plan.variant !== undefined ? plan.variant : rng.pick((plan.legal ? legalVariantsOf(code) : variantsOf(code)).filter((v) => !spec.variantSources?.[v]));
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
  // A drawn source (a combination program, `buildCombo`) is never a recall-only one (SOURCE_AXES.files): those are the `--recall`
  // grids' shapes, and drawing them moved the combination grids' programs. Nor a grid-only one (`GRID_ONLY_SOURCES`).
  const source = plan.source === "draw" ? rng.pick(sourcesOf(code, variant, binding, context).filter((s) => !SOURCE_AXES.files.includes(s) && !GRID_ONLY_SOURCES.includes(s))) : plan.source ?? "local";
  if (!sourcesOf(code, variant, binding, context).includes(source)) throw new Error(`${code}/${variant}/${binding} cannot take source ${source}`);
  // A parent source renders the component from its parent, plainly; the mount axis is not crossed with it, but in the pair grids
  // (`--pairs mount-source`), which name both: the parent then creates the values and mounts the component as the mount says.
  const paired = plan.source !== undefined && plan.source !== "draw" && plan.mount !== undefined && plan.mount !== "root";
  const mount = parentSource(source) ? (paired ? plan.mount : "child") : plan.mount ?? "root";
  if (!MOUNTS.includes(mount)) throw new Error(`unknown mount ${mount}`);
  const sourced = source === "local" ? {} : { source, sourced: sigs.map((sig) => sig.name) };
  // LOADING_ON_OUTSIDE_HOLD's boundary: `<Loading on={t()}>` around a read of the async memo, and the button that writes the trigger.
  const loadingFor = plan.legal ? spec.legalLoading?.[context] : undefined;
  const underLoading = loadingFor === true || (Array.isArray(loadingFor) && loadingFor.includes(variant));
  // A census code with no two values of its own (`--census`) brings none to the boundary: the context makes its trigger and async memo.
  const heldOwn = placed.loading && sigs.length < 2 ? loadingSignals(kit.name) : [];
  const [held, heldAsync] = heldOwn.length ? heldOwn : sigs;
  // The same for `on`: a code with no `onAttribute` passes a value of the boundary's own.
  const onOwn = placed.loadingOn && !spec.onAttribute ? [signal(kit.name("t"), 1, "src")] : [];
  const view = placed.loadingOn ? [loadingView(null, placed.view, onOwn.length ? `${onOwn[0].name}()` : spec.onAttribute(variant, sigs[0].name))]
    : placed.loading ? [loadingView(held.name, [child(kit.name("v"), read(heldAsync.name))]), ...placed.view, button(kit.name("b"), [expr(write(held.name, 2))])]
    : underLoading ? [loadingView(null, placed.view)] : placed.view;
  return {
    sigs: [...sigs, ...heldOwn, ...onOwn, ...(placed.sigs ?? [])], body: placed.body, view, ...(placed.hoisted ? { hoisted: placed.hoisted } : {}), ...(placed.top ? { top: placed.top } : {}),
    shape: `${code}/${context}/${binding}/${layer}${mount === "root" || source !== "local" ? "" : `/${mount}`}${source === "local" ? "" : `/${source}`}${plan.callback ? `/cb:${plan.callback}` : ""}`,
    misuse: { code, context, variant, binding, layer, mount, ...sourced, ...(plan.callback ? { callback: plan.callback } : {}) },
  };
}
