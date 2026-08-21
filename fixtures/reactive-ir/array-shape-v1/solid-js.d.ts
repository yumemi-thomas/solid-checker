declare namespace JSX {
  interface IntrinsicElements {
    [name: string]: any;
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
}
