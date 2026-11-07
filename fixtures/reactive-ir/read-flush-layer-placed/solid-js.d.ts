declare namespace JSX {
  interface IntrinsicElements { i: {}; p: { children?: unknown; onClick?: unknown } }
  type Element = unknown;
}

declare module "solid-js" {
  // As the published rc.21 typings for what this fixture calls.
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [Accessor<T>, Setter<T>];
  export function createMemo<T>(fn: () => T | Promise<T>): () => T;
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
}
