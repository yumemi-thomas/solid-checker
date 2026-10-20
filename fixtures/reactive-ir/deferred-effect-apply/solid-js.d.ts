declare namespace JSX {
  // Published @solidjs/web@2.0.0-rc.13 types/jsx.d.ts ref signatures.
  type RefCallback<T> = (el: T) => void;
  type Ref<T> = T | RefCallback<T> | undefined | Ref<T>[];
  interface IntrinsicElements {
    button: { onClick?: () => void; children?: unknown; ref?: Ref<HTMLButtonElement> };
  }
  interface Element {}
}

// Signatures and their supporting types are verbatim published rc.13
// @solidjs/signals declarations, with JSDoc omitted. Only the signal value
// overload used here is retained. The refresh brand has a local declaration.
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
    equals?: false | ((prev: T, next: T) => boolean);
    ownedWrite?: boolean;
    unobserved?: () => void;
  }
  export function createSignal<T>(value: Exclude<T, Function>, options?: SignalOptions<T>): Signal<T>;
  export type NoInfer<T extends any> = [T][T extends any ? 0 : never];
  export type ComputeFunction<Prev, Next extends Prev = Prev> = (v: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  export type EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) | void;
  export type EffectBundle<Prev, Next extends Prev = Prev> = {
    effect: EffectFunction<Prev, Next>;
    error: (err: unknown, cleanup: () => void) => void;
  };
  interface BaseEffectOptions {
    name?: string;
  }
  export interface EffectOptions extends BaseEffectOptions {
    defer?: boolean;
    schedule?: boolean;
    sync?: boolean;
    transparent?: boolean;
  }
  export function createEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T> | EffectBundle<NoInfer<T>, T>, options?: EffectOptions): void;
  export function createRenderEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T>, options?: EffectOptions): void;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
  export interface Disposable {
    (): void;
  }
  export function onCleanup(fn: Disposable): Disposable;
}
