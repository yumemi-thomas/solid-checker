declare namespace JSX {
  interface IntrinsicElements { button: { ref?: unknown } }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
}
