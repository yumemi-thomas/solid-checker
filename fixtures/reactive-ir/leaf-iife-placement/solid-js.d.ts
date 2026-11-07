declare namespace JSX {
  interface IntrinsicElements { i: {} }
  interface Element {}
}

declare module "solid-js" {
  export function onSettled(callback: () => void | (() => void)): void;
  export function onCleanup(callback: () => void): void;
  export function createRoot<T>(callback: (dispose: () => void) => T): T;
}
