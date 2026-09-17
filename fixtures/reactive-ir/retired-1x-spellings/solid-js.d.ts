declare namespace JSX {
  interface IntrinsicElements { div: { children?: unknown } }
  interface Element {}
}

// A project mid-migration: the ambient declarations still describe 1.x, so
// TypeScript is happy. Only the bundled model of what solid-js 2.0 actually
// exports can catch these.
declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createResource<T>(fetcher: () => Promise<T>): [() => T | undefined, unknown];
  export function createComputed(fn: () => void): void;
  export function batch<T>(fn: () => T): T;
  export function onMount(fn: () => void): void;
}
