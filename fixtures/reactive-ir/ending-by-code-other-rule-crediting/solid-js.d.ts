declare namespace JSX {
  interface IntrinsicElements { p: { children?: unknown } }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute { children: {} }
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => T];
  export function createMemo<T>(compute: (v: undefined | T) => T): Accessor<T>;
}
