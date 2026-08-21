declare namespace JSX {
  interface IntrinsicElements { h1: { children?: any }; div: { children?: any } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (next: T) => void];
}
