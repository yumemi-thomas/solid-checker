declare namespace JSX {
  interface IntrinsicElements { p: {}; i: {} }
  interface Element {}
  interface ElementChildrenAttribute { children: {} }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(fn: () => T): () => T;
  export function lazy<T>(load: () => Promise<{ default: T }>): T;
  export function Loading(props: { fallback?: unknown; children?: unknown }): unknown;
}

declare module "@solidjs/web" {
  export function render(fn: () => unknown, el: unknown): void;
  export function clientOnly<T>(load: () => Promise<{ default: T }>): T;
}
