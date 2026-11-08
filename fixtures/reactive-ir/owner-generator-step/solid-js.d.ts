declare namespace JSX {
  interface IntrinsicElements { i: {} }
  interface Element {}
}

declare module "solid-js" {
  export type Owner = object;
  export function getOwner(): Owner | null;
  export function onCleanup(fn: () => void): void;
}
