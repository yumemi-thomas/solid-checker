declare namespace JSX {
  interface IntrinsicElements { p: { children?: unknown } }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute { children: {} }
}

// The primitives the cases call, as `@solidjs/signals@2.0.0-rc.21` declares them (a callback that returns a value other than a cleanup is rejected by tsc, so the cases
// return none).
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createMemo<T>(compute: (v: undefined | T) => T): Accessor<T>;
  export function createRoot<T>(compute: (dispose: () => void) => T): T;
  export function createTrackedEffect(callback: () => void | (() => void)): void;
  export function onCleanup(callback: () => void): void;
}

declare module "@solidjs/web" {
  export function render(code: () => JSX.Element, element: object): () => void;
}
