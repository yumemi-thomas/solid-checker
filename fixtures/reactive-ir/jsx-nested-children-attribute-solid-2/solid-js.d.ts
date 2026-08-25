// Trimmed to the things this fixture's claims depend on: the signal factory,
// `onCleanup`, `onSettled`, and the intrinsic elements it nests.
//
// `children?: unknown` is the same shape the sibling divergence fixtures use.
// It is wider than solid-js's own `children?: Element | undefined`
// (`Element = RenderedElement | ArrayElement | (string & {}) | number |
// boolean | null | undefined` in the real `solid-js@2.0.0-rc.0` typings), but
// every value this fixture writes to it -- a number-returning accessor call,
// a string-valued constant expression, and a comma expression ending in
// `null` -- is one the real, narrower type already accepts. So the width
// cannot manufacture a finding a real project could not also produce; see the
// README's oracle check for the same claim verified against the published
// packages rather than assumed from this stub.
//
// `onCleanup`'s return type is declared `void`, narrower than the real
// package's `Disposable`. A stub must never be looser than the real package;
// nothing here reads the return value (the ownership arm discards it through
// a comma expression), so narrowing it cannot hide a finding either.
//
// Why the comma expression rather than `children={onCleanup(() => {})}`
// directly: `Disposable` is not assignable to `Element`, so the bare call as
// a `children` value is a `tsc` error (TS2322) against the real typings --
// checked, not assumed; see the README. A stub loose enough to accept it
// would be exactly the trap AGENTS.md names.
// `id?: string` exists only for the spread arms to have something real to
// spread. The real `noscript` accepts it through `HTMLAttributes<HTMLElement>`
// along with everything else; one optional string is narrower than that, and a
// narrower element type cannot manufacture a finding.
declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: unknown };
    b: { children?: unknown };
    span: { children?: unknown };
    noscript: { children?: unknown; id?: string };
  }
  interface Element {}
}

declare module "solid-js" {
  type ComputeFunction<Prev, Next extends Prev = Prev> = (value: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  type EffectFunction<Prev, Next extends Prev = Prev> = (value: Next, previous?: Prev) => (() => void) | void;
  type EffectBundle<Prev, Next extends Prev = Prev> = { effect: EffectFunction<Prev, Next>; error: (error: unknown, cleanup: () => void) => void };
  export function createEffect<T>(compute: ComputeFunction<undefined | T, T>, effect: EffectFunction<T, T> | EffectBundle<T, T>, options?: { name?: string }): void;
  /** @deprecated The client runtime throws MISSING_EFFECT_FN. */
  export function createEffect<T>(compute: ComputeFunction<undefined | T, T>): never;
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function onCleanup(fn: () => void): void;
  // Byte-faithful to `@solidjs/signals@2.0.0-rc.0`'s
  // `onSettled(callback: () => void | (() => void))`. The leaf-owner arms'
  // whole claim is which calls inside that callback the checker reports, so a
  // stub that loosened the callback type -- to `() => unknown`, say -- would
  // manufacture cleanup-return defects no real project can produce. Do not
  // widen it; see `fixtures/reactive-ir/leaf-owner/solid-js.d.ts`, which
  // carries the same signature for the same reason.
  export function onSettled(callback: () => void | (() => void)): void;
}
