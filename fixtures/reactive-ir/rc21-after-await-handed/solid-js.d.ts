declare namespace JSX {
  interface IntrinsicElements { i: {} }
  interface Element {}
  interface ElementChildrenAttribute { children: {} }
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createMemo<T>(compute: () => T | PromiseLike<T>): Accessor<T>;
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => void];
  export function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function Loading(props: { fallback?: any; children?: any }): any;
}
declare module "@solidjs/web" {
  export function render(code: () => any, element: any): () => void;
}
