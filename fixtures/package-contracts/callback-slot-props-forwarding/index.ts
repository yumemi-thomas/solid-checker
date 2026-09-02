import { createMemo, createSignal, mergeProps } from "solid-js";
import { createStore } from "solid-js/store";

export interface LinkProps {
  href: string;
  target?: string;
}

// Negative, and the shape that made `@solidjs/meta@0.29.4`'s `Stylesheet`
// publish an `invoke` claim on its props parameter. `mergeProps` memoizes a
// merge source *if* that source is a function and copies it otherwise, so a
// props object forwarded into it is never invoked. No callback row.
export function Stylesheet(props: LinkProps): unknown {
  return mergeProps({ rel: "stylesheet" }, props);
}

// Negative, second merge position: the over-claim matched *every* argument
// index, so the props object has to stay claim-free at index 0 too.
export function WithDefaults(props: LinkProps): unknown {
  return mergeProps(props, { target: "_blank" });
}

// Positive, and what keeps the withdrawal a grounding rule rather than a
// blanket exemption for `mergeProps`: a merge source the declaration proves
// callable really is invoked, tracked, when the merged result is read.
export function WithLazyExtras(extras: () => LinkProps): unknown {
  return mergeProps({ rel: "stylesheet" }, extras);
}

// Positive, and the second answer the premise accepts: `Function` is the
// signature-less function supertype, which the compiler proves callable while
// leaving no signature, arity, or parameter type to read
// (`Callability::UntypedCallable`). That is a *positive* proof, not the absence
// of one, so it roots the claim exactly as a read signature does -- and
// `mergeProps` accepts it because its published parameter list is a bare
// variadic tuple.
export function WithOpaqueExtras(extras: Function): unknown {
  return mergeProps({ rel: "stylesheet" }, extras);
}

// Negative, and the 1.x half of `@solid-primitives/flux-store`'s
// `createFluxStore`. 1.x's `createStore(store?, options?)` has no compute form
// at any arity -- the 1.x dialect's own callback-slot table says so -- so
// neither of these invokes `initial`.
export function makeStore(initial: LinkProps): unknown {
  const [state] = createStore(initial);
  return state;
}

export function makeNamedStore(initial: LinkProps): unknown {
  const [state] = createStore(initial, { name: "link" });
  return state;
}

// Negative: 1.x `createSignal(() => value)` stores the function as the
// signal's value and never invokes it, so even a provably callable parameter
// gets no row here.
export function makeSignal(initial: () => LinkProps): unknown {
  const [value] = createSignal(initial);
  return value;
}

// Positive control: `createMemo`'s argument 0 is the compute in every 1.x
// overload, unconditionally. This row must survive, or the withdrawal above has
// taken the whole branch with it.
export function derive(compute: () => number): () => number {
  return createMemo(compute);
}
