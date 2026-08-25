// Trimmed to the three things this fixture's claims depend on: the signal
// factory, `onCleanup`, and the intrinsic elements it nests. All are
// byte-faithful in the respects a finding could rest on.
//
// `children?: unknown` is the same shape the sibling dialect fixtures use. It
// is wider than solid-js's own `children?: JSX.Element`, but every child this
// fixture writes -- a number- or string-valued accessor call, or a comma
// expression ending in `null` -- is one the published typing accepts, so the
// width manufactures no finding. Giving `br` children is likewise not a type
// error against the real typings: Solid types `br` as
// `HTMLAttributes<HTMLBRElement>`, and `DOMAttributes` carries `children` for
// every element, void or not. The same holds for `hr`
// (`HTMLAttributes<HTMLHRElement>`), and for `keygen` and `menuitem`, which both
// published typings still declare as ordinary intrinsic elements.
//
// `br`'s `id` is the one attribute the fixture sets, and it is narrowed to
// `string` exactly as the published `HTMLAttributes` declares it -- a rule's
// proof rests on that attribute being a legal dynamic attribute, so the
// signature stays faithful rather than being widened to `unknown`.
//
// `onCleanup`'s return type is declared `void`, which is *narrower* than either
// published signature (1.x returns the callback, `T`; 2.0 returns `Disposable`).
// The direction matters: a stub must never be looser than the real package,
// because that is how a fixture invents a defect no project can produce. Nothing
// here reads the return value -- the ownership arms discard it through a comma
// expression -- so narrowing it cannot hide a finding either, and it keeps one
// declaration byte-shared across the pair where the real signatures differ.
//
// Why the comma expression rather than `{onCleanup(...)}` directly: neither
// published return type is a `JSX.Element`, so the bare call as a child is a
// `tsc` error (TS2322) against both real packages. A stub loose enough to accept
// it would be exactly the trap AGENTS.md names -- see this pair's README for the
// oracle output.
declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: unknown };
    b: { children?: unknown };
    br: { children?: unknown; id?: string };
    hr: { children?: unknown };
    keygen: { children?: unknown };
    menuitem: { children?: unknown };
    noscript: { children?: unknown; id?: string };
    span: { children?: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function onCleanup(fn: () => void): void;
}
