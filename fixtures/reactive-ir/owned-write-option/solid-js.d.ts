declare module "solid-js" {
  export function createSignal<T>(value: T, options?: { name?: string; equals?: false; ownedWrite?: boolean }): [() => T, (value: T | ((previous: T) => T)) => void];
  export function createStore<T extends object>(value: T, options?: { name?: string }): [T, (update: (draft: T) => void) => void];
  export function createMemo<T>(compute: () => T): () => T;
}
