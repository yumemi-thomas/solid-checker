// Copied from the published 2.0.0-rc.21 typings, JSDoc dropped: `createSignal`'s plain-value overload with `Signal`,
// `SourceAccessor`, `Accessor` and `Setter` (`@solidjs/signals/dist/types/signals.d.ts`), `Refreshable` (`core/constants.d.ts`,
// its `$REFRESH` brand a local `unique symbol`), and `createStore`'s plain-value overload (`store/index.d.ts`, `Store<T> = T`),
// its setter narrowed to the draft form. No case passes options or a function.
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
  export function createSignal<T>(value: Exclude<T, Function>): Signal<T>;
  export function createStore<T extends object = {}>(initialValue: T): [get: T, set: (fn: (draft: T) => void) => void];
}
declare namespace JSX {
  type Element = any;
  interface IntrinsicElements {
    [name: string]: any;
  }
  interface ElementChildrenAttribute {
    children: {};
  }
}
