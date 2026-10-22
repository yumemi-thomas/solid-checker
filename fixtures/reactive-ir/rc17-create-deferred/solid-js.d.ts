// Declarations transcribed from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17.
//
// Byte-faithful where a claim rests on them, up to the `export declare` an
// ambient `declare module` block spells `export`:
//
// - `ComputeFunction` @solidjs/signals `dist/types/signals.d.ts:74`
// - `createDeferred`  solid-js `types/client/hydration.d.ts:292-297`
//                     (re-exported by `types/index.d.ts:9`), its options
//                     reduced to `object` as `createMemo`'s are below
//
// Reduced, and safe to reduce because no claim reads them back: `createSignal`
// keeps only its value overload and `createEffect` its two-function shape
// (rc.17 `dist/types/signals.d.ts:302` and `:427`), `createMemo`
// (`types/client/hydration.d.ts:192-197`) has `object` options like
// `createDeferred`, `NoInfer` is the built-in, and `SourceAccessor`'s
// `Refreshable` brand is dropped; and `JSX` itself.

declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export type ComputeFunction<Prev, Next extends Prev = Prev> = (v: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export const createMemo: {
    <T>(compute: ComputeFunction<NoInfer<T>, T>, options: object & {
        loadingValue: T;
    }): Accessor<T>;
    <T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, options?: object): Accessor<T>;
  };
  export const createDeferred: {
    <T>(compute: ComputeFunction<NoInfer<T>, T>, options: object & {
        loadingValue: T;
    }): Accessor<T>;
    <T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, options?: object): Accessor<T>;
  };
  export function createEffect<Next, Init = Next>(
    compute: (prev: Init | Next) => Next,
    effect: (value: Next, prev?: Next) => (() => void) | void,
    options?: object
  ): void;
}
