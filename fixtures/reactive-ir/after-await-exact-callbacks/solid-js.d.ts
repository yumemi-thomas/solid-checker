declare namespace JSX {
  interface IntrinsicElements { p: { children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => T];
  // rc.17: a memo's compute may return a promise.
  export function createMemo<T>(fn: () => T | Promise<T>): () => T;
}
