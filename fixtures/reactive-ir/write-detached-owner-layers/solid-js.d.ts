declare namespace JSX {
  interface IntrinsicElements {
    div: {};
  }
}

declare module "solid-js" {
  export interface Owner { readonly id?: string }
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(compute: () => T): () => T;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function createOwner(): Owner;
  export function getOwner(): Owner | null;
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
  export function untrack<T>(fn: () => T): T;
}
