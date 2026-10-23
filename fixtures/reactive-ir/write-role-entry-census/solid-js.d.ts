declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void; onDblClick?: () => void; children?: unknown };
    p: { children?: unknown };
  }
  interface Element {}
  interface ElementChildrenAttribute {
    children: {};
  }
}

// Every declaration a claim here rests on is copied verbatim from the
// published 2.0.0-rc.17 typings, JSDoc dropped: `createSignal`, `createMemo`
// and `createEffect` with the types they name from
// `@solidjs/signals/dist/types/signals.d.ts`, and `Refreshable` from
// `core/constants.d.ts` (its `$REFRESH` brand is a local `unique symbol`).
// `createSignal` keeps only its plain-value overload and `createMemo` only its
// options-optional one; no case passes a function or a `loadingValue`.
declare module "solid-js" {
  const $REFRESH: unique symbol;
  export type Refreshable<T> = T & {
    readonly [$REFRESH]: any;
  };
  export type Accessor<T> = () => T;
  export type SourceAccessor<T> = Refreshable<Accessor<T>>;
  export type Setter<in out T> = {
    <U extends T>(...args: undefined extends T ? [] : [value: Exclude<U, Function> | ((prev: T) => U)]): undefined extends T ? undefined : U;
    <U extends T>(value: (prev: T) => U): U;
    <U extends T>(value: Exclude<U, Function>): U;
    <U extends T>(value: Exclude<U, Function> | ((prev: T) => U)): U;
  };
  export type Signal<T> = [get: SourceAccessor<T>, set: Setter<T>];
  export interface SignalOptions<T> {
    name?: string;
    loc?: string;
    equals?: false | ((prev: T, next: T) => boolean);
    ownedWrite?: boolean;
    unobserved?: () => void;
  }
  export function createSignal<T>(value: Exclude<T, Function>, options?: SignalOptions<T>): Signal<T>;
  export type ComputeFunction<Prev, Next extends Prev = Prev> = (v: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  export type EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) | void;
  export type EffectBundle<Prev, Next extends Prev = Prev> = {
    effect: EffectFunction<Prev, Next>;
    error: (err: unknown, cleanup: () => void) => void;
  };
  interface BaseEffectOptions {
    name?: string;
    loc?: string;
  }
  export interface EffectOptions extends BaseEffectOptions {
    defer?: boolean;
    schedule?: boolean;
    sync?: boolean;
    transparent?: boolean;
  }
  export interface MemoOptions<T> {
    id?: string;
    name?: string;
    loc?: string;
    transparent?: boolean;
    _plumbing?: boolean;
    equals?: false | ((prev: T, next: T) => boolean);
    unobserved?: () => void;
    lazy?: boolean;
    sync?: boolean;
    loadingValue?: T;
  }
  export type NoInfer<T extends any> = [T][T extends any ? 0 : never];
  export function createMemo<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, options?: MemoOptions<T>): SourceAccessor<T>;
  export function createEffect<const T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T> | EffectBundle<NoInfer<T>, T>, options?: EffectOptions): void;
}
