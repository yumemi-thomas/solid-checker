// Declarations transcribed from solid-js@2.0.0-rc.9, which re-exports each of
// these names from @solidjs/signals@2.0.0-rc.9 (`types/index.d.ts:1`).
//
// Byte-faithful, because every claim in this fixture rests on them:
//
// - `until`, `UntilOptions`, `Truthy`
//                          `dist/types/signals.d.ts:608`, `:537-544`, `:536`
//                          (all three new in rc.9; rc.3 has no `until`)
// - `untrack`              `dist/types/core/core.d.ts:105`
// - `action`               `dist/types/core/action.d.ts:71`
// - `createTrackedEffect`  `dist/types/signals.d.ts:443`, except that its
//                          `BaseEffectOptions` is `object` here
//
// ("Byte-faithful" up to the one token an ambient `declare module` block
// forbids: the published files spell `export declare function`.)
//
// Reduced, and safe to reduce because no claim reads them back: `createSignal`
// keeps only its value overload (`:263`), `createMemo` its optional-options
// overload without `ComputeFunction`'s async arms (`:298`), `createEffect` its
// two-function shape (`:367`), the option types are `object`, and `JSX` is
// this file's. None is looser for any argument written here.

declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: any };
    button: { onClick?: unknown; children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export type Truthy<T> = Exclude<T, false | 0 | 0n | "" | null | undefined>;
  export interface UntilOptions {
      /** Reject with `TimeoutError` if the predicate has not turned truthy within
       * this many milliseconds. Strongly recommended when the confirming truth
       * arrives over a transport that can drop (sockets, subscriptions). */
      timeout?: number;
      /** Reject with `signal.reason` on abort. */
      signal?: AbortSignal;
  }
  export function until<T>(fn: () => T, options?: UntilOptions): Promise<Truthy<T>>;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createMemo<T>(compute: (prev: T | undefined) => T, options?: object): Accessor<T>;
  export function createEffect<Next, Init = Next>(
    compute: (prev: Init | Next) => Next,
    effect: (value: Next, prev?: Next) => (() => void) | void,
    options?: object
  ): void;
  export function createTrackedEffect(compute: () => void | (() => void), options?: object): void;
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
}
