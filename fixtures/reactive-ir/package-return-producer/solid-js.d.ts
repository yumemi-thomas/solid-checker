declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(compute: () => T): () => T;
  export function createStore<T extends object>(value: T): [T, (update: (draft: T) => void) => void];
}
