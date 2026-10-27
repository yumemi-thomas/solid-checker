declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void };
    div: {};
  }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(compute: () => T): () => T;
  export function createRoot<T>(init: (dispose: () => void) => T): T;
  export function untrack<T>(fn: () => T): T;
}
