declare namespace JSX {
  interface IntrinsicElements { div: {} }
}

declare module "solid-js" {
  // `createSignal`'s value overload of the published `@solidjs/signals` 2.0.0-rc.21 declarations (options omitted).
  export function createSignal<T>(value: Exclude<T, Function>): [() => T, (value: T | ((prev: T) => T)) => T];
}
