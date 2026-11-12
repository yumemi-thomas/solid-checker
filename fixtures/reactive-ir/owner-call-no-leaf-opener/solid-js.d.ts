declare namespace JSX {
  interface IntrinsicElements { p: { children?: unknown } }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute { children: {} }
}

// The primitives the cases call, as `@solidjs/signals@2.0.0-rc.21` declares them.
declare module "solid-js" {
  export function createTrackedEffect(callback: () => void | (() => void)): void;
  export function onCleanup(callback: () => void): void;
}

declare module "@solidjs/web" {
  export function render(code: () => JSX.Element, element: object): () => void;
}
