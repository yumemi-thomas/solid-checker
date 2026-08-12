declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

declare module "solid-js" {
  export function createEffect<T>(fn: (prev?: T) => T | void, value?: T): void;
  export function createRoot<T>(fn: (dispose: () => void) => T): T;
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
}
