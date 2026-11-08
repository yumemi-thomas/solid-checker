declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
  export function createMemo<T>(compute: (prev?: T) => T): Accessor<T>;
  export function snapshot<T>(value: T): T;
  export function For<T extends readonly any[], U extends JSX.Element>(props: {
    each: T | undefined | null | false;
    fallback?: JSX.Element;
    keyed?: true;
    children: (item: T[number], index: Accessor<number>) => U;
  }): JSX.Element;
  export function For<T extends readonly any[], U extends JSX.Element>(props: {
    each: T | undefined | null | false;
    fallback?: JSX.Element;
    keyed: false;
    children: (item: Accessor<T[number]>, index: number) => U;
  }): JSX.Element;
}
declare namespace JSX {
  type Element = any;
  interface IntrinsicElements {
    [name: string]: any;
  }
}
