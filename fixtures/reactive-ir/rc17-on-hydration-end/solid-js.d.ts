// Declarations transcribed from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17.
//
// Byte-faithful, because every claim in this fixture rests on them, up to the
// `export declare` an ambient `declare module` block spells `export`:
//
// - `onHydrationEnd` solid-js `types/client/hydration.d.ts:144` (re-exported
//                    by `types/index.d.ts:9`; `types/server/hydration.d.ts:37`
//                    is the same)
// - `onCleanup`      @solidjs/signals `dist/types/signals.d.ts:47`, its
//                    `Disposable` (`dist/types/core/types.d.ts:3-5`) spelled
//                    as the call signature it is
//
// Reduced, and safe to reduce because no claim reads them back: `createSignal`
// keeps only its value overload and `createEffect` its two-function shape
// (rc.17 `dist/types/signals.d.ts:302` and `:427`); and `JSX` itself.

declare namespace JSX {
  interface IntrinsicElements {}
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createEffect<Next, Init = Next>(
    compute: (prev: Init | Next) => Next,
    effect: (value: Next, prev?: Next) => (() => void) | void,
    options?: object
  ): void;
  export function onCleanup(fn: () => void): () => void;
  export function onHydrationEnd(callback: () => void): void;
}
