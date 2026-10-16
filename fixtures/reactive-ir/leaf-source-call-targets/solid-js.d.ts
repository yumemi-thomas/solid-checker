declare namespace JSX { interface IntrinsicElements { div: {} } }
declare module "solid-js" {
  // These callback returns match @solidjs/signals' published RC.13 shapes.
  export function createTrackedEffect(callback: () => void | (() => void)): void;
  export function onCleanup(callback: () => void): void;
  export function createMemo<T>(compute: () => T): () => T;
  export function createSignal<T>(value: T): [() => T, (value: T | ((previous: T) => T)) => T];
  export function createEffect<T>(compute: () => T, apply: (value: T, previous?: T) => (() => void) | void): void;
}
