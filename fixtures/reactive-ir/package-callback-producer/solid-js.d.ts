declare module "solid-js" {
  export function createMemo<T>(compute: () => T): () => T;
  export function onSettled(callback: () => unknown): void;
}

declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
  }
}
