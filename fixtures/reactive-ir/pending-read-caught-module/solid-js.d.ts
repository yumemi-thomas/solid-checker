declare namespace JSX {
  interface IntrinsicElements { i: {}; p: { children?: unknown } }
  type Element = unknown;
}

declare module "solid-js" {
  // As the published rc.21 typings for what this fixture calls.
  export type Accessor<T> = () => T;
  export function createMemo<T>(fn: () => T | Promise<T>, options?: { loadingValue?: T }): Accessor<T>;
}

declare module "@solidjs/web" {
  export type MountableElement = Element | Document | ShadowRoot | DocumentFragment | Node;
  export function render(code: () => JSX.Element, element: MountableElement, init?: JSX.Element): () => void;
}
