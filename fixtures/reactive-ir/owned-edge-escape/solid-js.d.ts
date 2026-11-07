declare namespace JSX {
  interface IntrinsicElements { i: {}; button: { onClick?: () => void; children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function action(fn: any): () => void;
  export function onCleanup(callback: () => void): void;
}
