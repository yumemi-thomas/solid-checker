declare namespace JSX {
  interface IntrinsicElements { i: {}; button: { onClick?: () => void; children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function onCleanup(callback: () => void): void;
}
