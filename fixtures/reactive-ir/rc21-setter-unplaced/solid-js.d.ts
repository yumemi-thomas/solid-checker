// Declarations transcribed from @solidjs/signals@2.0.0-rc.21, which solid-js@2.0.0-rc.21 re-exports
// (`dist/types/signals.d.ts`, `dist/types/core/constants.d.ts`), options reduced to the keys this fixture passes.
declare namespace JSX {
  interface IntrinsicElements {
    i: { children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  const $REFRESH: unique symbol;
  type Refreshable<T> = T & { readonly [$REFRESH]: any };
  type Accessor<T> = () => T;
  type SourceAccessor<T> = Refreshable<Accessor<T>>;
  type Setter<in out T> = {
    <U extends T>(...args: undefined extends T ? [] : [value: Exclude<U, Function> | ((prev: T) => U)]): undefined extends T ? undefined : U;
    <U extends T>(value: (prev: T) => U): U;
    <U extends T>(value: Exclude<U, Function>): U;
    <U extends T>(value: Exclude<U, Function> | ((prev: T) => U)): U;
  };
  type Signal<T> = [get: SourceAccessor<T>, set: Setter<T>];
  type ComputeFunction<Prev, Next extends Prev = Prev> = (v: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  interface SignalOptions<T> { ownedWrite?: boolean }
  interface MemoOptions<T> { name?: string }
  export function createSignal<T>(value: Exclude<T, Function>, options?: SignalOptions<T>): Signal<T>;
  export function createMemo<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, options?: MemoOptions<T>): SourceAccessor<T>;
  export function refresh<T>(target: Refreshable<T>): Promise<T extends (...args: any) => infer V ? V : T>;
}
