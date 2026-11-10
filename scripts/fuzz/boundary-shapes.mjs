// The boundary-wrapper shapes (ADR 0935): a USER component that wraps `props.children` in a boundary the checker's cover walks consult.
//
// The mounts put a boundary directly around the child (mounts.mjs `loading`, `errored`; print.mjs `async-loading`, the context source). No generated
// client program had a component of the program's own whose body wraps `props.children`, so the walks that look into such a body were never asked:
// async_context.rs `loading_cover_at` takes a wrapper whose body holds a member named `children` inside a `<Loading>` as covering what it is given
// (`children_in_loading`: the property's spelling, not that the object is the wrapper's `props`), and stored_throw.rs `handled_above` and
// context_census.rs consult an `<Errored>` and a context provider the same way. The shapes, per boundary and per form (the JSX element, the
// `create*Boundary` call):
//
//   real     `props.children` inside the boundary: covered, handled, provided
//   outside  the children beside the boundary, a literal inside it: not covered
//   shadow   a foreign object's `.children` inside, the real children beside: not covered, though the property is spelled `children`
//
// Recall contexts only, base shapes (the plain shape of each variant, misuse.mjs `fireShape`), on the two codes the boundaries decide:
// ASYNC_OUTSIDE_LOADING_BOUNDARY (an async read in the children under `<Loading>`) and CONTEXT_NOT_FOUND (a `useContext(Ctx)` in the children, under a
// `<Ctx value>` provider, or thrown into an `<Errored>` whose fallback takes the error: ADR 0893, the program's own handling).
import { decl, domExpr, fn, jsx, num, prim, ret, wrapEl } from "./model.mjs";

/** `{ [context]: { boundary, form, case } }`. */
export const WRAP_CONTEXTS = {};
// The JSX form of `Loading` is the proof-log agent's (branch claude/proof-log-phase-b, mounts.mjs `WRAP_MOUNTS`: wrap-real, wrap-alias, wrap-foreign,
// wrap-outside, the rediscovery programs of the `loading_cover_at` fix): not repeated here, so no program is written twice. This file holds what
// that lacks: the `createLoadingBoundary` call form, `Errored` (both forms) and the context provider.
const FORMS = { loading: ["call"], errored: ["jsx", "call"], provider: ["jsx"] };
for (const [boundary, forms] of Object.entries(FORMS)) {
  for (const form of forms) for (const kase of ["real", "outside", "shadow"]) WRAP_CONTEXTS[`wrap-${boundary}${form === "call" ? "call" : ""}-${kase}`] = { boundary, form, case: kase };
}
export const WRAP_CONTEXT_NAMES = Object.keys(WRAP_CONTEXTS);

/** The codes the family is placed for, and the boundaries each is decided by. */
export const WRAP_CODES = { ASYNC_OUTSIDE_LOADING_BOUNDARY: ["loading"], CONTEXT_NOT_FOUND: ["provider", "errored"] };

/**
 * The placement of `stmts` for a `wrap-*` context (`{ body, view }`), or undefined for any other. The site is in `InnerN`, a component of C1's body that the
 * view renders as the wrapper's child, `<WrapN><InnerN /></WrapN>`: the boundary walks (async_context.rs `loading_cover_at`, stored_throw.rs
 * `handled_above`, context_census.rs) follow the call sites of the function that holds the site, and a call site that is a JSX child of a user component is
 * where they read that component's body. (A site in an IIFE in C1's own JSX has C1's unknown caller above it and no walk is decisive: the first cut
 * of this file, every cell uncertifiable.) A pending read goes behind a memo `InnerN` creates and reads in its JSX (`{m1()}`), as the `memo` context writes
 * it: the proof-log agent found (branch claude/proof-log-phase-b) that a read inside an IIFE hole is unplaced.
 */
export function wrapContext(context, stmts, kit) {
  const spec = WRAP_CONTEXTS[context];
  if (!spec) return undefined;
  const id = kit.name("w");
  const inner = `Inner${id.replace(/^\D+/, "")}`;
  let component;
  if (spec.boundary === "loading") {
    const m = kit.name("m");
    component = fn([decl(m, prim("memo", fn([...stmts, ret(num(1))]))), ret(domExpr(`<p data-view="v${id.replace(/^\D+/, "")}">{${m}()}</p>`))]);
  } else {
    component = fn([...stmts, ret(jsx())]);
  }
  return { body: [decl(inner, component)], view: [wrapEl(id, spec.boundary, spec.form, spec.case, [], { inner })] };
}

/**
 * `{ [code]: { misuse, legal } }` for SPEC: `real` is legal (the boundary holds what is given: covered, provided, handled), `outside` and `shadow` are
 * misuse (the runtime reports the code there).
 */
export function wrapPlacement() {
  const placement = {};
  for (const [code, boundaries] of Object.entries(WRAP_CODES)) {
    const names = WRAP_CONTEXT_NAMES.filter((context) => boundaries.includes(WRAP_CONTEXTS[context].boundary));
    placement[code] = { misuse: names.filter((context) => WRAP_CONTEXTS[context].case !== "real"), legal: names.filter((context) => WRAP_CONTEXTS[context].case === "real") };
  }
  return placement;
}
