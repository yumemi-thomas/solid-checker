declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

declare module "solid-js" {
  export function createMemo<T>(compute: () => Promise<T>): () => T;
  export function createMemo<T>(compute: () => AsyncIterable<T> | T): () => T;
  export function createSignal<T>(compute: () => Promise<T>): [() => T, (value: T) => void];
  export function createStore<T extends object>(compute: () => Promise<T>, seed: T): [T, (value: T) => void];
  export function createProjection<T extends object>(compute: () => Promise<T>, seed: T): T;
  export function onSettled(callback: () => void): void;
  export function Loading(props: { fallback: JSX.Element; children: JSX.Element }): JSX.Element;
}

declare module "@solidjs/web" {
  export function dynamic<T extends (props: {}) => JSX.Element>(
    source: () => Promise<T> | T,
  ): T;
}

declare function fetchUser(): Promise<{ name: string }>;
