declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: unknown }; p: {} }
  interface Element {}
}

declare module "solid-js" {
  export function createMemo<T>(fn: () => T): () => T;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function createRoot<T>(fn: () => T): T;
  export function createTrackedEffect(fn: () => void): void;
  export function onSettled(fn: () => void): void;
  export function onFlush(callback: () => void): void;
}
