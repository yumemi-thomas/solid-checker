declare namespace JSX {
  interface IntrinsicElements { div: {}; button: { onClick?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function createMemo<T>(fn: () => T): () => T;
  export function createEffect<T>(fn: (prev?: T) => T, value?: T): void;
  export function onMount(fn: () => void): void;
}

declare module "solid-js/store" {
  export function createStore<T extends object>(value: T): [T, (next: Partial<T>) => void];
}

declare module "./persist" {
  export function makePersisted<T>(signal: T): T;
}
