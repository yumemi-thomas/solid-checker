declare namespace JSX {
  interface IntrinsicElements {
    div: { title?: unknown };
    button: { onClick?: unknown };
    span: { title?: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function createMemo<T>(fn: () => Promise<T>): () => T;
  export function createMemo<T>(fn: () => T): () => T;
  // 2.0 folds the store APIs into core.
  export function createStore<T extends object>(value: T): [T, (next: Partial<T>) => void];
  export function onSettled(callback: () => void): void;
}
