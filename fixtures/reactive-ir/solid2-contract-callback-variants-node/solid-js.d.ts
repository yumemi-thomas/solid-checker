declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, never>;
  }
  interface Element {}
}

declare module "solid-js" {
  export function createMemo<T>(compute: () => T): () => T;
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function repeat<T>(
    count: () => number,
    mapFn: (index: number) => T,
    options?: { fallback?: () => any; from?: () => number | undefined },
  ): () => T[];
  export function createErrorBoundary<T, U>(
    body: () => T,
    fallback: (error: () => unknown, reset: () => void) => U,
  ): () => T | U;
  export function createLoadingBoundary<T, U>(
    body: () => T,
    fallback: () => U,
    options?: { on?: () => any },
  ): () => T | U;
}
