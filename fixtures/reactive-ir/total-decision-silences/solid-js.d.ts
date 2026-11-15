declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: unknown };
    div: { children?: unknown };
    button: { onClick?: () => void; children?: unknown };
  }
  type Element = unknown;
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => T];
  // rc.17 `types/client/flow.d.ts`: `on?: any`.
  export function Loading(props: { fallback?: JSX.Element; on?: any; children?: JSX.Element }): JSX.Element;
  // The optimistic pair: a plain initial value is a synchronous source (no async work to be before).
  export function createOptimistic<T>(v: T): [() => T, (n: T | ((p: T) => T)) => T];
}
