declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
}
