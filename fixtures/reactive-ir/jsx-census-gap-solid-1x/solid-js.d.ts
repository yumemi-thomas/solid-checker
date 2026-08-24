// Trimmed to the two things this fixture's claim depends on: the signal
// factory and the intrinsic elements it nests. Both are byte-faithful in the
// respects a finding could rest on.
//
// `children?: unknown` is the same shape the sibling dialect fixtures use. It
// is wider than solid-js's own `children?: JSX.Element`, but every child this
// fixture writes -- a string-valued accessor call -- is one the published
// typing accepts, so the width manufactures no finding. Nesting `<head>`
// inside `<div>` is not a type error against the real typings either: Solid's
// JSX namespace types attributes and children, never document structure.
declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: unknown };
    head: { children?: unknown };
    noscript: { children?: unknown };
    title: { children?: unknown; id?: string };
    span: { children?: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
}
