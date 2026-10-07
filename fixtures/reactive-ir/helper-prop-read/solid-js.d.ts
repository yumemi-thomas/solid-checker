declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: unknown) => void; ref?: unknown; children?: unknown; [attribute: string]: unknown };
    main: { children?: unknown; [attribute: string]: unknown };
    p: { children?: unknown; [attribute: string]: unknown };
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
// 2.0.0-rc.13 typings, JSDoc dropped: `createSignal`, `createEffect`,
// `untrack` and their helper types from
// `@solidjs/signals/dist/types/signals.d.ts`; `createStore` (its plain-value
// overload), `Store`, `StoreSetter` and `NoFn` from `store/`; `merge` from
// `store/utils.d.ts`; `For` and `Show` from `solid-js/types/client/flow.d.ts`.
// `StoreOptions` and `EffectOptions` keep only `name`.
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
  export interface EffectOptions {
    name?: string;
  }
  export function createEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T> | EffectBundle<NoInfer<T>, T>, options?: EffectOptions): void;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;

  export type Store<T> = T;
  export type StoreSetter<T> = (fn: (state: T) => T | void) => void;
  export interface StoreOptions {
    name?: string;
  }
  export type NoFn<T> = T extends Function ? never : T;
  export function createStore<T extends object = {}>(initialValue: NoFn<T> | Store<NoFn<T>>, options?: StoreOptions): [get: Store<T>, set: StoreSetter<T>];

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

  type SolidElement = JSX.Element;
  type NonZeroParams<T extends (...args: any[]) => any> = Parameters<T>["length"] extends 0 ? never : T;
  type ConditionalRenderCallback<T> = (item: Accessor<NonNullable<T>>) => SolidElement;
  type KeyedConditionalRenderCallback<T> = (item: NonNullable<T>) => SolidElement;
  export function For<T extends readonly any[], U extends SolidElement>(props: {
    each: T | undefined | null | false;
    fallback?: SolidElement;
    keyed?: true;
    children: (item: T[number], index: Accessor<number>) => U;
  }): SolidElement;
  export function For<T extends readonly any[], U extends SolidElement>(props: {
    each: T | undefined | null | false;
    fallback?: SolidElement;
    keyed: false;
    children: (item: Accessor<T[number]>, index: number) => U;
  }): SolidElement;
  export function Show<T>(props: {
    when: T | undefined | null | false;
    keyed: true;
    fallback?: SolidElement;
    children: SolidElement;
  }): SolidElement;
  export function Show<T, F extends KeyedConditionalRenderCallback<T>>(props: {
    when: T | undefined | null | false;
    keyed: true;
    fallback?: SolidElement;
    children: NonZeroParams<F>;
  }): SolidElement;
  export function Show<T>(props: {
    when: T | undefined | null | false;
    keyed?: false;
    fallback?: SolidElement;
    children: SolidElement;
  }): SolidElement;
  export function Show<T, F extends ConditionalRenderCallback<T>>(props: {
    when: T | undefined | null | false;
    keyed?: false;
    fallback?: SolidElement;
    children: NonZeroParams<F>;
  }): SolidElement;
}
