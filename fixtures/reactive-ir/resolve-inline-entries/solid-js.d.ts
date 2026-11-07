declare namespace JSX {
  interface IntrinsicElements { i: {}; p: { children?: unknown; onClick?: unknown } }
  type Element = unknown;
}

declare module "solid-js" {
  // As the published rc.21 typings for what this fixture calls.
  export function createMemo<T>(fn: () => T | Promise<T>): () => T;
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
  export function resolve<T>(fn: () => T): Promise<T>;
  export function until<T>(fn: () => T): Promise<T>;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
}
