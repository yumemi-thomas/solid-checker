// Byte-faithful to solid-js@2.0.0-rc.0 for the two signatures this fixture's
// proof depends on. `flatten` is `@solidjs/signals`
// dist/types/boundaries.d.ts:171 and `createEffect` is
// dist/types/signals.d.ts:367; both are re-exported by solid-js's root
// declaration. A stub that widened either would manufacture a finding no real
// project can produce.
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type SourceAccessor<T> = Accessor<T>;
  export type Setter<T> = (value: T) => T;
  export type Signal<T> = [get: SourceAccessor<T>, set: Setter<T>];
  export type ComputeFunction<Prev, Next extends Prev = Prev> = (
    v: Prev
  ) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  export type EffectFunction<Prev, Next extends Prev = Prev> = (
    v: Next,
    p?: Prev
  ) => (() => void) | void;

  export function createSignal<T>(value: Exclude<T, Function>): Signal<T>;
  export function createMemo<T>(compute: ComputeFunction<undefined | T, T>): Accessor<T>;
  export function createEffect<T>(
    compute: ComputeFunction<undefined | NoInfer<T>, T>,
    effectFn: EffectFunction<NoInfer<T>, T>
  ): void;
  export function flatten(
    children: any,
    options?: {
      skipNonRendered?: boolean;
      doNotUnwrap?: boolean;
    }
  ): any;
}
