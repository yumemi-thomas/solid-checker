declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: unknown; children?: unknown }; i: { children?: unknown }; div: { children?: unknown; "data-view"?: unknown }; p: { children?: unknown; title?: unknown; "data-view"?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Store<T> = T;
  export type StoreSetter<T> = (fn: (draft: T) => T | void) => void;
  export function createStore<T extends object>(value: T, options?: { shallow?: boolean }): [Store<T>, StoreSetter<T>];
  export function createStore<T extends object>(fn: (draft: T) => T | Promise<T> | void, seed: T): [Store<T>, StoreSetter<T>];
  export function createProjection<T extends object>(fn: (draft: T) => T | Promise<T> | void, seed: T): Store<T>;
  export function createOptimistic<T>(fn: () => T | Promise<T>, options?: { loadingValue?: T }): [Accessor<T>, (value: T) => void];
  export function createOptimisticStore<T extends object>(fn: (draft: T) => T | Promise<T> | void, seed: T): [Store<T>, StoreSetter<T>];
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => void];
  export function createMemo<T>(fn: () => T | Promise<T>, options?: { loadingValue?: T }): Accessor<T>;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function createTrackedEffect(fn: () => void): void;
  export function createRoot<T>(fn: () => T): T;
  export function untrack<T>(fn: () => T, label?: string | false): T;
  export function snapshot<T>(store: T): T;
  export function merge<A extends object, B extends object>(a: A, b: B): A & B;
  export function merge<A extends object>(a: A): A;
  export function latest<T>(fn: () => T): T;
  export function isPending(fn: () => unknown): boolean;
  export function onCleanup(fn: () => void): void;
  export function Loading(props: { fallback?: unknown; children?: unknown }): JSX.Element;
}

declare module "@solidjs/web" {
  export function render(code: () => unknown, element: unknown): () => void;
}

declare module "@solidjs/signals" {
  export function markRaw<T extends object>(value: T): T;
}
