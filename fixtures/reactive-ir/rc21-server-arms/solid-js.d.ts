declare namespace JSX {
  interface IntrinsicElements { p: {} }
  interface Element {}
  interface ElementChildrenAttribute { children: {} }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createSignal<T>(fn: () => T): [() => T, (value: T) => void];
  export function createMemo<T>(fn: () => T): () => T;
  export function Loading(props: { on?: any; fallback?: unknown; children?: unknown }): unknown;
}

declare module "@solidjs/web" {
  export const isServer: boolean;
  export const isDev: boolean;
  export function renderToStream(fn: () => unknown): unknown;
}
