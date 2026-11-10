declare namespace JSX {
  interface IntrinsicElements { div: {} }
}

declare module "solid-js" {
  // Byte-faithful to `@solidjs/signals`'s `createTrackedEffect(compute: () => void | (() => void), options?)`, `onCleanup` and `untrack`.
  export function createTrackedEffect(callback: () => void | (() => void)): void;
  export function onCleanup(callback: () => void): void;
  export function untrack<T>(fn: () => T): T;
}
