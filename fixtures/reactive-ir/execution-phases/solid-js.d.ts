declare namespace JSX {
  interface IntrinsicElements { div: { children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(compute: () => T): () => T;
  export function createEffect<T>(compute: () => T, apply: (value: T) => unknown): void;
  export function createTrackedEffect(compute: () => unknown): void;
  export function onSettled(callback: () => unknown): void;
  export function untrack<T>(callback: () => T): T;
}
