declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: unknown) => void; children?: unknown; [attribute: string]: unknown };
    main: { children?: unknown };
    div: { children?: unknown; [attribute: string]: unknown };
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

  // `merge`, `omit` and their types, copied from `@solidjs/signals@2.0.0-rc.13`
  // `dist/types/store/utils.d.ts` (JSDoc and `declare`, which an ambient module
  // cannot repeat, dropped).
  type DistributeOverride<T, F> = T extends undefined ? F : T;
  type Override<T, U> = T extends any ? U extends any ? {
      [K in keyof T]: K extends keyof U ? DistributeOverride<U[K], T[K]> : T[K];
  } & {
      [K in keyof U]: K extends keyof T ? DistributeOverride<U[K], T[K]> : U[K];
  } : T & U : T & U;
  type OverrideSpread<T, U> = T extends any ? {
      [K in keyof ({
          [K in keyof T]: any;
      } & {
          [K in keyof U]?: any;
      } & {
          [K in U extends any ? keyof U : keyof U]?: any;
      })]: K extends keyof T ? Exclude<U extends any ? U[K & keyof U] : never, undefined> | T[K] : U extends any ? U[K & keyof U] : never;
  } : T & U;
  type Simplify<T> = T extends any ? {
      [K in keyof T]: T[K];
  } : T;
  type _Merge<T extends unknown[], Curr = {}> = T extends [
      infer Next | (() => infer Next),
      ...infer Rest
  ] ? _Merge<Rest, Override<Curr, Next>> : T extends [...infer Rest, infer Next | (() => infer Next)] ? Override<_Merge<Rest, Curr>, Next> : T extends [] ? Curr : T extends (infer I | (() => infer I))[] ? OverrideSpread<Curr, I> : Curr;
  export type Merge<T extends unknown[]> = Simplify<_Merge<T>>;
  export function merge<T extends unknown[]>(...sources: T): Merge<T>;
  export type Omit<T, K extends readonly (keyof T)[]> = {
      [P in keyof T as Exclude<P, K[number]>]: T[P];
  };
  export function omit<T extends Record<any, any>, K extends readonly (keyof T)[]>(props: T, ...keys: K): Omit<T, K>;
  export function omit<T extends Record<any, any>>(props: T, hidden: (key: keyof T & (string | symbol)) => boolean): Partial<T>;
}
