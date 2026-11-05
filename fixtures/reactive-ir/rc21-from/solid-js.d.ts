declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: unknown }; p: {} }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(fn: () => T | AsyncIterable<T>): () => T;
  export function from<T>(source: (next: (value: T) => void) => (() => void) | void): AsyncIterable<T>;
}
