declare namespace JSX {
  interface IntrinsicElements { div: { children?: any } }
  interface Element {}
}

declare module "solid-js" {
  export interface MemoOptions<T> { loadingValue?: T; ssrSource?: "server" | "hybrid" | "client" }
  export function createMemo<T>(compute: () => Promise<T>, options?: MemoOptions<T>): () => T;
  export function createMemo<T>(compute: () => T, options?: MemoOptions<T>): () => T;
  export function Loading(props: { fallback?: JSX.Element; children?: JSX.Element | JSX.Element[] }): JSX.Element;
}

declare module "@solidjs/web" {
  export function httpStatus(code: number, text?: string): void;
  export function httpHeader(name: string, value: string, options?: { append?: boolean }): void;
}
