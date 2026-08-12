declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function createEffect<T>(compute: (prev?: T) => T, apply?: (value: T) => void): void;
  export function onSettled(fn: () => void): void;
  export function children(fn: () => unknown): () => unknown;
  export function For<T>(props: {
    each: readonly T[];
    children: (item: T, index: () => number) => unknown;
  }): unknown;
  export function Repeat(props: {
    count: number;
    children: (index: number) => unknown;
  }): unknown;
}
