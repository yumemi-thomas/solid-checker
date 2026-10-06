declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(value: T | (() => T)): [() => T, (value: T | ((prev: T) => T)) => T];
  export function createMemo<T>(compute: (prev?: T) => T): () => T;
  export function createStore<T extends object>(value: T): [T, (next: Partial<T>) => void];
}
