declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createEffect<T>(fn: (previous?: T) => T, value?: T): void;
  export function createReaction(onInvalidate: () => void): (tracking: () => void) => void;
  export function onCleanup(fn: () => void): void;
  export function createResource<T>(
    fetcher: () => Promise<T> | T,
  ): [() => T | undefined, unknown];
  export function createResource<S, T>(
    source: () => S,
    fetcher: (source: S) => Promise<T> | T,
  ): [() => T | undefined, unknown];
  export function mapArray<T, U>(
    list: () => T[],
    map: (item: T, index: () => number) => U,
  ): () => U[];
  export function indexArray<T, U>(
    list: () => T[],
    map: (item: () => T, index: number) => U,
  ): () => U[];
}

declare module "solid-js/store" {
  export function createStore<T>(value: T): [T, (value: T) => void];
}
