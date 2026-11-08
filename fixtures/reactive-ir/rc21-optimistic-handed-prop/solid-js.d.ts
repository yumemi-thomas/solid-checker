declare namespace JSX {
  interface IntrinsicElements { p: {}; button: { onClick?: (event: MouseEvent) => void } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createOptimistic<T>(value: (() => T | Promise<T>) | T): [() => T, (value: T) => void];
}
