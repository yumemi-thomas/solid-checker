declare namespace JSX {
  interface IntrinsicElements { i: {} }
  interface Element {}
}

declare module "solid-js" {
  // As the published rc.21 typings for what this fixture calls: a store
  // setter takes a draft callback, `createEffect(compute, apply)`.
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
  export function createEffect<T>(compute: () => T, apply: (value: T) => void | (() => void)): void;
}
