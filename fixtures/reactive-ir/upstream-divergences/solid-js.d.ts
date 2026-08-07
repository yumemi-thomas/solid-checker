declare namespace JSX {
  interface IntrinsicElements {
    [name: string]: any;
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function mergeProps(...sources: object[]): object;
  export function createReaction(onInvalidate: () => void): (track: () => void) => void;
  export function createResource<T>(fetcher: () => Promise<T>): [() => T | undefined, object];
}
