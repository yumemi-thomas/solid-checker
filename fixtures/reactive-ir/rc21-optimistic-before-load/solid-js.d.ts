declare namespace JSX {
  interface IntrinsicElements { p: {} }
  interface Element {}
}

declare module "solid-js" {
  export function createRoot<T>(fn: () => T): T;
  export function createOptimistic<T>(value: (() => T | Promise<T>) | T): [() => T, (value: T) => void];
  export function createOptimisticStore<T extends object>(fn: () => T | Promise<T>, seed: T): [T, (fn: (draft: T) => void) => void];
  export function createOptimisticStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
}
