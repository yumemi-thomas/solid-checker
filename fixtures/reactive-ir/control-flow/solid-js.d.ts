declare namespace JSX {
  interface IntrinsicElements { div: {}; span: {} }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function Show<T>(props: { when: T; children: (value: Accessor<T>) => JSX.Element }): JSX.Element;
  export function For<T>(props: {
    each: readonly T[];
    keyed?: boolean | ((item: T) => unknown);
    children: (item: any, index: any) => JSX.Element;
  }): JSX.Element;
}
