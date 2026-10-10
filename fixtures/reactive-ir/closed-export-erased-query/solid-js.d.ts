declare namespace JSX {
  interface IntrinsicElements { div: {} }
}

declare module "solid-js" {
  export function createSignal<T>(value: Exclude<T, Function>): [() => T, (value: T | ((prev: T) => T)) => T];
}
