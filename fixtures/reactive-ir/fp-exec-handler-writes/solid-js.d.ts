declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void; children?: unknown };
    main: { children?: unknown };
    div: { children?: unknown };
  }
  type RenderedElement = object & {
    readonly call?: never;
    readonly apply?: never;
    readonly bind?: never;
  };
  type Element = RenderedElement | ArrayElement | (string & {}) | number | boolean | null | undefined;
  interface ArrayElement extends Array<Element> {}
  interface ElementChildrenAttribute {
    children: {};
  }
}

// Every declaration a claim here rests on is copied from the published
// 2.0.0-rc.9 typings, JSDoc dropped: `createSignal` with its helper types,
// `createMemo` (the overload without a required loadingValue),
// `createTrackedEffect`, `onCleanup` and `onSettled` from
// `@solidjs/signals/dist/types/signals.d.ts`, and `Disposable` from
// `core/types.d.ts`. `createSignal` keeps only its plain-value overload.
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
  export interface MemoOptions<T> {
    id?: string;
    name?: string;
    transparent?: boolean;
    equals?: false | ((prev: T, next: T) => boolean);
    unobserved?: () => void;
    lazy?: boolean;
    sync?: boolean;
    loadingValue?: T;
  }
  export function createMemo<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, options?: MemoOptions<T>): SourceAccessor<T>;
  export interface Disposable {
    (): void;
  }
  export interface BaseEffectOptions {
    name?: string;
  }
  export function onCleanup(fn: Disposable): Disposable;
  export function createTrackedEffect(compute: () => void | (() => void), options?: BaseEffectOptions): void;
  export function onSettled(callback: () => void | (() => void)): void;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
}
