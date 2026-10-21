declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
}

declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: unknown };
  }
  interface Element {}
}
