// `createStore`'s plain-value overload as published in 2.0.0-rc.21
// (`@solidjs/signals/dist/types/store/index.d.ts`, `Store<T> = T`), its setter
// narrowed to the draft form; no case passes options or a function.
declare module "solid-js" {
  export function createStore<T extends object = {}>(initialValue: T): [get: T, set: (fn: (draft: T) => void) => void];
}
declare namespace JSX {
  type Element = any;
  interface IntrinsicElements {
    [name: string]: any;
  }
}
