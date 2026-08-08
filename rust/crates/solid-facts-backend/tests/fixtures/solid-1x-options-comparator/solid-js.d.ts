declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(
    fn: (previous?: T) => T,
    value?: T,
    options?: { equals?: false | ((previous: T, next: T) => boolean) },
  ): () => T;
}
