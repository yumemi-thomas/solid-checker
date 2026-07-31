declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

// Deliberately permissive. This project is about SC7005, which reads import
// declarations, not resolution -- so every module declares every name the
// fixture imports and TypeScript never objects. The checker's answer comes
// from the generated export index, and if these declarations were the source
// of truth the rule would agree with whatever the fixture claimed.
declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function createMemo<T>(fn: () => T): () => T;
  export function onMount(fn: () => void): void;
  export function createStore<T extends object>(v: T): [T, (n: Partial<T>) => void];
  export function unwrap<T>(v: T): T;
  export function Portal(props: {}): any;
  export function Show(props: {}): any;
  export function render(fn: () => any, el: any): () => void;
  export type Store<T> = T;
  export type Accessor<T> = () => T;
}

declare module "solid-js/store" {
  export function createStore<T extends object>(v: T): [T, (n: Partial<T>) => void];
  export function unwrap<T>(v: T): T;
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export type Store<T> = T;
}

declare module "solid-js/web" {
  export function Portal(props: {}): any;
  export function Show(props: {}): any;
  export function render(fn: () => any, el: any): () => void;
  export function newInPatch(): boolean;
}

declare module "@my/ui" {
  export function createStore<T extends object>(v: T): [T, (n: Partial<T>) => void];
}
