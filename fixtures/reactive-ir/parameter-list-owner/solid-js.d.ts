declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void };
    div: {};
    p: {};
  }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T | ((previous: T) => T)) => void];
  export function createMemo<T>(compute: () => T): () => T;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function onSettled(callback: () => void): void;
  export function onCleanup(cleanup: () => void): void;
}
