declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

declare module "solid-js" {
  export function createEffect<T>(compute: ((prev?: T) => T) | undefined, apply?: T | ((value: T) => void)): void;
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function batch<T>(fn: () => T): T;
  export function onMount(fn: () => void): void;
  export function createReaction(fn: () => void): (tracking: () => void) => void;
  export function onCleanup(fn: () => void): void;
  export function createMemo<T>(fn: () => T, options?: { sync?: boolean }): () => T;
}

declare module "solid-js/store" {
  export function createStore<T extends object>(value: T): [T, (next: Partial<T>) => void];
}
