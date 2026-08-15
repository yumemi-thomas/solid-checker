declare namespace JSX {
  interface IntrinsicElements { div: { children?: any }; button: { onClick?: unknown; children?: any } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function createMemo<T>(fn: () => T): () => T;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function createTrackedEffect(fn: () => void): void;
  export function untrack<T>(fn: () => T): T;
  export function resolve<T>(fn: () => T): Promise<T>;
}
