declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: unknown };
    button: { onClick?: () => void; children?: unknown };
  }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute { children: {} }
}

// The primitives the cases call, as `@solidjs/signals@2.0.0-rc.21` declares them: an effect's apply returns nothing or a cleanup.
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createMemo<T>(compute: (v: undefined | T) => T): Accessor<T>;
  export function createRoot<T>(compute: (dispose: () => void) => T): T;
  export function createEffect<T>(
    compute: () => T,
    apply: (value: T, previous?: T) => (() => void) | void,
  ): void;
}

declare module "@solidjs/web" {
  export function render(code: () => JSX.Element, element: object): () => void;
}
