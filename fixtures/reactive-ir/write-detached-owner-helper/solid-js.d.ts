declare namespace JSX {
  interface IntrinsicElements {
    div: {};
  }
}

declare module "solid-js" {
  export interface Owner { readonly id?: string }
  export type Disposable = () => void;
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createOwner(): Owner;
  export function createRoot<T>(init: ((dispose: () => void) => T) | (() => T)): T;
  export function getOwner(): Owner | null;
  export function onCleanup(fn: Disposable): Disposable;
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
  export function untrack<T>(fn: () => T): T;
}
