declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: unknown }; p: {} }
  interface Element {}
}

declare module "solid-js" {
  export type Store<T> = T;
  export type StoreSetter<T> = (fn: (draft: T) => void) => void;
  export function createStore<T extends object>(value: T, options?: { shallow?: boolean }): [Store<T>, StoreSetter<T>];
  export function createStore<T extends object>(fn: (draft: T) => T | Promise<T> | void, seed: T): [Store<T>, StoreSetter<T>];
  export function deep<T extends object>(store: T): T;
  export function snapshot<T>(store: T): T;
  export function createMemo<T>(fn: () => T): () => T;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
}
