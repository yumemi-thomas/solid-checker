// Declarations transcribed from solid-js@2.0.0-rc.3, which re-exports each of
// these names from @solidjs/signals@2.0.0-rc.3 (`types/index.d.ts:1`).
//
// The claim rests on what is *absent*: neither package exports `until` on rc.3
// (it arrives with rc.5), so `import { until } from "solid-js"` is TS2305
// against this stub and against the published rc.3 typings alike. Do not add
// `until` here.
//
// Byte-faithful: `untrack` (`dist/types/core/core.d.ts:75`). Reduced, and safe
// to reduce because no claim reads them back: `createSignal` keeps only its
// value overload (`dist/types/signals.d.ts:263`) and `createMemo` its
// optional-options overload without `ComputeFunction`'s async arms (`:298`),
// and `JSX` is this file's.

declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createMemo<T>(compute: (prev: T | undefined) => T, options?: object): Accessor<T>;
}
