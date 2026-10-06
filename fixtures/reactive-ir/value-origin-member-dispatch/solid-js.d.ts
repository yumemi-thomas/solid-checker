declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createStore<T extends object>(value: T): [T, (next: Partial<T>) => void];
}
