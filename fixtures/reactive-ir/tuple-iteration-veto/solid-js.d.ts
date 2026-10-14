declare namespace JSX {
  interface IntrinsicElements { main: { children?: unknown }; }
  type RenderedElement = object & {
    readonly call?: never;
    readonly apply?: never;
    readonly bind?: never;
  };
  type Element = RenderedElement | ArrayElement | (string & {}) | number | boolean | null | undefined;
  interface ArrayElement extends Array<Element> {}
  interface ElementChildrenAttribute { children: {}; }
}

// Plain-value createSignal overload and all types it depends on, copied from
// @solidjs/signals 2.0.0-rc.13 (JSDoc removed).
declare module "solid-js" {
  const $REFRESH: unique symbol;
  export type Refreshable<T> = T & { readonly [$REFRESH]: any };
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
  // rc.13 core declarations; only unused EffectOptions fields are reduced.
  export interface EffectOptions { name?: string; }
  export type ContextProviderComponent<T> = (props: { value: T; children: JSX.Element }) => JSX.Element;
  export interface Context<T> extends ContextProviderComponent<T> { id: symbol; defaultValue: T | undefined; }
  export function createContext<T>(defaultValue?: T, options?: EffectOptions): Context<T>;
  export function useContext<T>(context: Context<T>): T;
  // rc.13 plain-value overload, Store, StoreSetter and NoFn; StoreOptions
  // omits unused fields. No tuple-slot or setter signature is loosened.
  export type Store<T> = T;
  export type StoreSetter<T> = (fn: (state: T) => T | void) => void;
  export interface StoreOptions { name?: string; }
  export type NoFn<T> = T extends Function ? never : T;
  export function createStore<T extends object = {}>(initialValue: NoFn<T> | Store<NoFn<T>>, options?: StoreOptions): [get: Store<T>, set: StoreSetter<T>];
}
