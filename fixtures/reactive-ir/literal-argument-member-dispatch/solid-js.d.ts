declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
}
