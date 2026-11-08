declare namespace JSX {
  interface IntrinsicElements { p: {} }
  interface Element {}
  interface ElementChildrenAttribute { children: {} }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(fn: () => T): () => T;
  export function createTrackedEffect(fn: () => void): void;
}
