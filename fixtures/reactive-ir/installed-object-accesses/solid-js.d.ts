declare namespace JSX {
  interface IntrinsicElements { i: {} }
  interface Element {}
}

declare module "solid-js" {
  export type Store<T> = Readonly<T>;
  export type SetStoreFunction<T> = (fn: (draft: T) => void) => void;
  export function createStore<T extends object>(value: T): [Store<T>, SetStoreFunction<T>];
  export function merge<T extends object[]>(...sources: T): T[number];
}
