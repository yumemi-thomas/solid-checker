// Solid 1.x declarations. Verified against solid-js 1.9.14.
declare namespace JSX {
  interface IntrinsicElements { div: { children?: any } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createEffect<T>(fn: (previous?: T) => T, value?: T): void;
  export function createMemo<T>(fn: (previous?: T) => T, value?: T): () => T;
  export function onMount(fn: () => void): void;
  export function untrack<T>(fn: () => T): T;
  export function createRoot<T>(fn: (dispose: () => void) => T): T;
}
