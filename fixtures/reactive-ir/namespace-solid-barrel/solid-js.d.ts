declare namespace JSX {
  interface IntrinsicElements { p: { children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => T];
  export function createMemo<T>(fn: () => T): () => T;
}
