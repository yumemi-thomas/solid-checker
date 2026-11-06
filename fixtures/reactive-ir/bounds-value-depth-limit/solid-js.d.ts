declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
  export function createMemo<T>(compute: (prev?: T) => T | Promise<T>): Accessor<T>;
}
declare namespace JSX {
  type Element = any;
  interface IntrinsicElements {
    [name: string]: any;
  }
}
