declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

declare module "solid-js" {
  export interface MemoOptions<T> { loadingValue?: T; ssrSource?: "server" | "hybrid" | "client" }
  export function createMemo<T>(compute: () => T, options?: MemoOptions<T>): () => T;
}

declare function computeWidth(): number;
