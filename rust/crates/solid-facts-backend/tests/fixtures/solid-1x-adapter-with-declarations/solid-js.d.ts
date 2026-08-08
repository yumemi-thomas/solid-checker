declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function mapArray<T, U>(
    list: () => T[],
    map: (item: T, index: () => number) => U,
  ): () => U[];
}
