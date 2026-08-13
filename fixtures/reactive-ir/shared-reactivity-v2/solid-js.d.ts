declare namespace JSX {
  interface IntrinsicElements { div: { title?: unknown }; button: { onClick?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function createMemo<T>(fn: () => T): () => T;
  // 2.0 folds the store APIs into core: createStore is a root export here,
  // unlike the 1.x fixtures that import it from solid-js/store.
  export function createStore<T extends object>(value: T): [T, (next: Partial<T>) => void];
}
