declare namespace JSX {
  interface IntrinsicElements { div: { children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createSignal<T>(fn: () => T): [Accessor<T>, (value: T) => void];
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => void];
}
