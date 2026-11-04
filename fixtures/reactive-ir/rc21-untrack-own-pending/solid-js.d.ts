declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: unknown }; p: {} }
  interface Element {}
}

declare module "solid-js" {
  export type Store<T> = T;
  export function createStore<T extends object>(fn: (draft: T) => T | Promise<T> | void, seed: T): [Store<T>, (fn: (draft: T) => void) => void];
  export function createProjection<T extends object>(fn: (draft: T) => T | Promise<T> | void, seed: T): Store<T>;
  export function createMemo<T>(fn: () => T | Promise<T>): () => T;
  export function createRoot<T>(fn: () => T): T;
  export function untrack<T>(fn: () => T, label?: string | false): T;
}
