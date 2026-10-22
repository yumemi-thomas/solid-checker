// Declarations transcribed from solid-js@2.0.0-rc.17, @solidjs/signals@2.0.0-rc.17
// and @solidjs/web@2.0.0-rc.17.
//
// Byte-faithful, because every claim in this fixture rests on them:
//
// - `dispatchAsInteraction`
//                  @solidjs/web `types/client.d.ts:203` (re-exported by
//                  `types/index.d.ts:3`)
// - `untrack`      @solidjs/signals `dist/types/core/core.d.ts:167`
//
// ("Byte-faithful" up to the one token an ambient `declare module` block
// forbids: the published files are modules, so they spell `export declare
// function`, and inside the block that is `export function`.)
//
// Reduced, and safe to reduce because no claim reads them back: `createSignal`
// keeps only its value overload, `createMemo` its one-compute shape and
// `createEffect` its two-function shape (rc.17 `dist/types/signals.d.ts:302`,
// `:358` and `:427`), without the `ComputeFunction`/`EffectFunction` inference
// machinery, which is no looser for any argument written here; and `JSX`
// itself.

declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: MouseEvent) => void; children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createMemo<T>(compute: (prev: T | undefined) => T, options?: object): Accessor<T>;
  export function createEffect<Next, Init = Next>(
    compute: (prev: Init | Next) => Next,
    effect: (value: Next, prev?: Next) => (() => void) | void,
    options?: object
  ): void;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
}

declare module "@solidjs/web" {
  export function dispatchAsInteraction<T>(e: Event, fn: () => T): T;
}
