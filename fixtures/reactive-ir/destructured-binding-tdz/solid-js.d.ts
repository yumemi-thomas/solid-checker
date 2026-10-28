declare namespace JSX {
  interface IntrinsicElements { p: { children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function createMemo<T>(fn: () => T): () => T;
  // `Store<T> = T` on @solidjs/signals rc.7 and later (the audited rc.17 too).
  export function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
}
