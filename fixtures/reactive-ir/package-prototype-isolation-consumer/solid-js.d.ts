declare namespace JSX {
  interface IntrinsicElements { div: Record<string, unknown>; }
  interface Element {}
  interface ElementChildrenAttribute { children: {}; }
}
declare module "solid-js" {
  interface BaseEffectOptions { name?: string; }
  // Published rc.13 signature, unchanged; no return-valued callback loophole.
  export function createTrackedEffect(compute: () => void | (() => void), options?: BaseEffectOptions): void;
  export function createMemo<T>(compute: () => T): () => T;
}
