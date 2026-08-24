// Trimmed to the two things this fixture's claim depends on: the signal
// factory and the intrinsic elements it writes. Both are byte-faithful in the
// respects a finding could rest on.
//
// `children?: unknown` is the same shape the sibling dialect fixtures use. It
// is wider than solid-js's own `children?: JSX.Element`, but every child this
// fixture writes -- a number-valued accessor call -- is one the published
// typing accepts, so the width manufactures no finding. Giving `br` children
// is likewise not a type error against the real typings: Solid types `br` as
// `HTMLAttributes<HTMLBRElement>`, and `DOMAttributes` carries `children`
// for every element, void or not.
// `textContent` is narrowed to `string` exactly as the published
// `HTMLAttributes` declares it, because the retraction arm's proof rests on it
// being a legal dynamic attribute rather than an unknown one.
declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: unknown };
    br: { children?: unknown };
    span: { children?: unknown; textContent?: string };
    noscript: { children?: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
}
