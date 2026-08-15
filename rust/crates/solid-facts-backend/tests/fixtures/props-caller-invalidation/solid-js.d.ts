declare namespace JSX {
  interface IntrinsicElements { div: { title?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
}
