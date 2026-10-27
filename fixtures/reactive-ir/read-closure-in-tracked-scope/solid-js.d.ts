declare namespace JSX {
  interface IntrinsicElements {
    div: {};
  }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T | ((previous: T) => T)) => void];
  export function createMemo<T>(compute: () => T): () => T;
}
