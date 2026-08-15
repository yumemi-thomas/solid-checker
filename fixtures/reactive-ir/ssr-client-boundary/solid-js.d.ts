declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

declare module "solid-js" {
  export interface MemoOptions<T> { loadingValue?: T; ssrSource?: "server" | "hybrid" | "client" }
  export interface StoreOptions { seedLoadingValue?: boolean; ssrSource?: "server" | "hybrid" | "client" }
  export function createMemo<T>(compute: () => T, options?: MemoOptions<T>): () => T;
  export function createProjection<T extends object>(compute: (draft: T) => void, seed: T, options?: StoreOptions): T;
  export function Loading(props: { fallback: JSX.Element; children: JSX.Element }): JSX.Element;
}

declare module "@solidjs/web" {
  export function renderToStream(fn: () => JSX.Element): unknown;
}
