declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void; children?: unknown };
    div: { children?: unknown };
  }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute {
    children: {};
  }
}

// Every declaration here is copied from the published 2.0.0-rc.13 typings,
// JSDoc dropped: `Accessor`, `SourceAccessor`, `Setter`, `Signal`,
// `ComputeFunction`, `EffectFunction`, `EffectBundle`, `EffectOptions`,
// `SignalOptions`, `MemoOptions`, `NoInfer`, `Truthy`, `UntilOptions`,
// `createSignal` (value overload), `createMemo`, `createEffect`, `resolve`,
// `until` and `onSettled` from `@solidjs/signals/dist/types/signals.d.ts`,
// `createRoot` from `core/owner.d.ts`,
// `action` from `core/action.d.ts`, `$REFRESH`/`Refreshable` from
// `core/constants.d.ts`. `UntilOptions.signal` is narrowed to the DOM
// `AbortSignal` the published conditional type selects under the DOM lib.
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
    transparent?: boolean;
  }
  export interface SignalOptions<T> {
    name?: string;
    equals?: false | ((prev: T, next: T) => boolean);
    ownedWrite?: boolean;
    unobserved?: () => void;
  }
  export interface MemoOptions<T> {
    id?: string;
    name?: string;
    transparent?: boolean;
    _plumbing?: boolean;
    equals?: false | ((prev: T, next: T) => boolean);
    unobserved?: () => void;
    lazy?: boolean;
    sync?: boolean;
    loadingValue?: T;
  }
  export type NoInfer<T extends any> = [T][T extends any ? 0 : never];
  export type Truthy<T> = Exclude<T, false | 0 | 0n | "" | null | undefined>;
  export interface UntilOptions {
    timeout?: number;
    signal?: AbortSignal;
  }
  export function createSignal<T>(value: Exclude<T, Function>, options?: SignalOptions<T>): Signal<T>;
  export function createMemo<T>(compute: ComputeFunction<NoInfer<T>, T>, options: MemoOptions<T> & {
    loadingValue: T;
  }): SourceAccessor<T>;
  export function createMemo<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, options?: MemoOptions<T>): SourceAccessor<T>;
  export function createEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T> | EffectBundle<NoInfer<T>, T>, options?: EffectOptions): void;
  export function resolve<T>(fn: () => T): Promise<T>;
  export function until<T>(fn: () => T, options?: UntilOptions): Promise<Truthy<T>>;
  export function onSettled(callback: () => void | (() => void)): void;
  export function createRoot<T>(init: ((dispose: () => void) => T) | (() => T), options?: {
    id?: string;
    transparent?: boolean;
  }): T;
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
}
