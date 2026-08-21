declare namespace JSX {
  interface IntrinsicElements { h1: { children?: any }; div: { children?: any } }
  interface Element {}
}
declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (next: T) => void];
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function createRoot<T>(fn: (dispose: () => void) => T): T;
}
