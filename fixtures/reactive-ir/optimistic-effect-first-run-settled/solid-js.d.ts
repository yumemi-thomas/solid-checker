declare namespace JSX {
  interface IntrinsicElements { div: {}; p: {}; button: { onClick?: () => void } }
  interface Element {}
}

declare module "solid-js" {
  export function createOptimistic<T>(value: (() => T | Promise<T>) | T, options?: { loadingValue?: T }): [() => T, (value: T) => void];
  export function createOptimisticStore<T extends object>(fn: () => T | Promise<T>, seed: T): [T, (fn: (draft: T) => void) => void];
  export function createEffect<T>(compute: () => T, apply: (value: T) => void, options?: { defer?: boolean }): void;
  export function onSettled(callback: () => void | (() => void)): void;
  export function createRenderEffect<T>(compute: () => T, apply: (value: T) => void): void;
}

declare module "@solidjs/web" {
  export function render(code: () => JSX.Element, element: Element): () => void;
}
