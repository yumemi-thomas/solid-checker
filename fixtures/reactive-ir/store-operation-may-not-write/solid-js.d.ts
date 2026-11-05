declare module "solid-js" {
  export function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
}
