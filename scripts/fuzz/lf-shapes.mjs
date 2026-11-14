// ADR 0978: `Loading` entered other than as a JSX tag, and what covers a read under it. Solid's `Loading(props)` is a component function, so a program may
// call it (`Loading({ ... })`, through an alias, a namespace, a wrapper, `createComponent(Loading, props)`, `Loading.call(null, props)`, `[Loading][0](props)`), name it
// as a value (`dynamicComponent(() => Loading)`, `<Dynamic component={Loading}>`, a tag held by a prop) or build the boundary with `createLoadingBoundary`. A cell crosses
// the way the boundary is entered with what is asked of it:
//
//   lf-<form>         LOADING_ON_OUTSIDE_HOLD (misuse; `clb-on` is `createLoadingBoundary(children, fallback, { on })`): `on` holds a read of the trigger, a sibling reads the async memo outside, a button re-arms it.
//                     Chrome warns at the sibling read. SC5006 must answer (a proof or an uncertifiable row) for the form, never nothing.
//   lf-acc-<form>     LOADING_ON_ACCESSOR (misuse): `on` is a function (SC5007 must answer, as for the JSX tag).
//   lf-cover-<form>   ASYNC_OUTSIDE_LOADING_BOUNDARY (legal): the read is in the boundary's `children` getter or function and no read is outside. Chrome is quiet,
//                     so a violation at the read is a false positive (the walk read the getter as outside).
//   lf-fb-<form>      ASYNC_OUTSIDE_LOADING_BOUNDARY (misuse): the read is in the boundary's `fallback`, which renders outside the hold. Chrome warns.
//
// Forms (`LF_FORMS`): the call and the value references. The site of a cover or fallback cell is the read itself (the code's `use` node, written as the hole's expression), so the verdict is placed.
// Recall contexts, the direct binding bare (`lfShape`), not crossed by the census.
import { child, fn, iife, num, propEl, ret } from "./model.mjs";

/** How the boundary is entered, with the cells each form takes. */
export const LF_FORMS = {
  hand: { hold: true, acc: true, cover: true, call: "Loading" },
  "alias-hand": { hold: true, cover: true, call: "Hold", used: "Loading as Hold" },
  "ns-hand": { hold: true, cover: true, call: "S.Loading", namespace: true },
  "wrapper-hand": { hold: true, cover: true, call: "LfWrap", helper: "wrapper" },
  createcomponent: { hold: true, acc: true, cover: true, call: "createComponent(Loading, ", close: ")", also: ["createComponent"] },
  dotcall: { hold: true, acc: true, cover: true, call: "Loading.call(null, ", close: ")" },
  array: { hold: true, acc: true, cover: true, call: "[Loading][0]" },
  iife: { hold: true, cover: true, call: "(() => Loading(", close: "))()" },
  dynamic: { hold: true, acc: true, cover: true, element: "LfDyn", helper: "dynamic" },
  "dynamic-el": { hold: true, acc: true, element: "Dynamic component={Loading}", web: ["Dynamic"], closeTag: "Dynamic" },
  "unresolved-param": { hold: true, acc: true, element: "LfGate as={Loading}", helper: "gate", closeTag: "LfGate" },
  clb: { cover: true, fallback: true },
  "clb-on": { hold: true },
  jsx: { fallback: true },
};
export const LF_FORM_NAMES = Object.keys(LF_FORMS);

export const LF_HOLD = "LOADING_ON_OUTSIDE_HOLD";
export const LF_ACCESSOR = "LOADING_ON_ACCESSOR";
export const LF_COVER = "ASYNC_OUTSIDE_LOADING_BOUNDARY";
const withFlag = (flag) => Object.keys(LF_FORMS).filter((form) => LF_FORMS[form][flag]);
/** `{ [context]: "misuse" | "legal" }`. */
export const LF_CONTEXTS = {
  ...Object.fromEntries(withFlag("hold").map((form) => [`lf-${form}`, "misuse"])),
  ...Object.fromEntries(withFlag("acc").map((form) => [`lf-acc-${form}`, "misuse"])),
  ...Object.fromEntries(withFlag("cover").map((form) => [`lf-cover-${form}`, "legal"])),
  // The fallback: a JSX attribute, a hand call's `fallback` getter, and the second argument of `createLoadingBoundary`.
  ...Object.fromEntries(["jsx", "hand", "clb"].map((form) => [`lf-fb-${form}`, "misuse"])),
};
export const LF_CONTEXT_NAMES = Object.keys(LF_CONTEXTS);

const NAME = /^lf-(acc|cover|fb)-(.+)$|^lf-(.+)$/;
/** `{ kind: "hold" | "acc" | "cover" | "fb", form }` of an `lf-*` context name, or null. */
export const lfParts = (context) => {
  if (!LF_CONTEXT_NAMES.includes(context)) return null;
  const match = NAME.exec(context);
  return match[1] ? { kind: match[1], form: match[2] } : { kind: "hold", form: match[3] };
};

/** The base shapes an `lf-*` cell takes: the direct binding, no layer. */
export function lfShape(context, shape) {
  return !lfParts(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/**
 * The placement of `stmts` for an `lf-*` context, or undefined for any other. Returns the usual `{ body, view }` and `lf`: the form (`kind` `hold` and `acc` ride
 * the existing `loading` / `loadingOn` boundary; `cover` puts the site in the boundary's children, `fb` in its fallback). misuse.mjs `buildOne` makes the view.
 */
export function lfContext(context, stmts, kit) {
  const parts = lfParts(context);
  if (!parts) return undefined;
  const site = child(kit.name("v"), iife(fn([...stmts, ret(num(1))])));
  // The hole of a cover or fallback cell is the read itself, as the Chrome programs write it (`<p>{slow()}</p>`): the site is the read, so the walk's answer is the verdict and
  // not the unplaced read an IIFE's hole leaves.
  const held = () => ({ body: [], view: [stmts.length === 1 && stmts[0].k === "expr" ? propEl("p", stmts[0].e, "a", true) : site] });
  switch (parts.kind) {
    case "hold": return { body: [], view: [site], loading: true, loadingForm: parts.form };
    case "acc": return { body: [], view: [site], loadingOn: true, loadingForm: parts.form };
    case "cover": return { ...held(), loadingSlot: { form: parts.form, slot: "children" } };
    case "fb": return { ...held(), loadingSlot: { form: parts.form, slot: "fallback" } };
    default: throw new Error(`lf: no kind ${parts.kind}`);
  }
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function lfPlacement() {
  const names = (kind, side) => LF_CONTEXT_NAMES.filter((name) => LF_CONTEXTS[name] === side && lfParts(name).kind === kind);
  return {
    [LF_HOLD]: { misuse: names("hold", "misuse"), legal: [] },
    [LF_ACCESSOR]: { misuse: names("acc", "misuse"), legal: [] },
    [LF_COVER]: { misuse: names("fb", "misuse"), legal: names("cover", "legal") },
  };
}
