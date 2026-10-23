declare namespace JSX {
  interface IntrinsicElements {
    p: { title?: unknown };
    button: { onClick?: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
}
