declare namespace JSX {
  interface IntrinsicElements { li: {}; ul: {}; b: {}; p: {} }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function untrack<T>(fn: () => T): T;
  export function For<T>(props: { each: readonly T[]; children: (item: T, index: () => number) => unknown }): unknown;
}
