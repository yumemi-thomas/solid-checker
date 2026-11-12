declare namespace JSX {
  interface IntrinsicElements { p: { children?: unknown } }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute { children: {} }
}

// `createMemo` and `createOptimistic` as the fixtures of this claim call them: `rc21-optimistic-before-load` (`createOptimistic`) and `caught-handed-read` (`createMemo`,
// copied from the published 2.0.0-rc.9 typings, JSDoc dropped).
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createMemo<T>(compute: (v: undefined | T) => PromiseLike<T> | T): Accessor<T>;
  export function createOptimistic<T>(value: (() => T | Promise<T>) | T): [() => T, (value: T) => void];
}
