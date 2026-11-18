// The rc.21 declarations this fixture uses, as `@solidjs/signals` 2.0.0-rc.21 publishes them (dist/types/signals.d.ts,
// dist/types/store/index.d.ts:33-35, dist/types/store/store.d.ts:5), reduced to the overloads written here.
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T | ((prev: T) => T)) => T;
  export function createSignal<T>(value: T): [Accessor<T>, Setter<T>];
  export type SetStoreFunction<T> = (fn: (draft: T) => T | void) => void;
  export function createStore<T extends object = {}>(initialValue: T): [T, SetStoreFunction<T>];
  export function createStore<T extends object = {}>(fn: (draft: T) => void | T, seed: T): [T, SetStoreFunction<T>];
}
