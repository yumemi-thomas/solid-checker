declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

declare module "solid-js" {
  export interface MemoOptions<T> { loadingValue?: T; ssrSource?: "server" | "hybrid" | "client" }
  export interface StoreOptions { seedLoadingValue?: boolean; ssrSource?: "server" | "hybrid" | "client" }
  export function createMemo<T>(compute: () => Promise<T>, options?: MemoOptions<T>): () => T;
  export function createMemo<T>(compute: () => AsyncIterable<T> | T, options?: MemoOptions<T>): () => T;
  export function createSignal<T>(compute: () => Promise<T>): [() => T, (value: T) => void];
  export function createStore<T extends object>(compute: () => Promise<T>, seed: T, options?: StoreOptions): [T, (value: T) => void];
  export function createProjection<T extends object>(compute: () => Promise<T>, seed: T, options?: StoreOptions): T;
  export function latest<T>(read: () => T): T | undefined;
  export function isPending(read: () => unknown): boolean;
  export function onSettled(callback: () => void): void;
  export function refresh(target: unknown): void;
  export function Loading(props: { fallback: JSX.Element; children: JSX.Element }): JSX.Element;
}

declare module "@solidjs/web" {
  export function dynamic<T extends (props: {}) => JSX.Element>(
    source: () => Promise<T> | T,
  ): T;
}

declare function fetchUser(): Promise<{ name: string }>;
