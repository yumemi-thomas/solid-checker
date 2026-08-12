declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

declare module "solid-js" {
  export function createMemo<T>(fn: () => T): () => T;
  export function createSignal<T>(value: T): [() => T, (next: T) => void];
  export function refresh(target: unknown): void;
  export function affects(target: unknown, keys?: string[]): void;
}
