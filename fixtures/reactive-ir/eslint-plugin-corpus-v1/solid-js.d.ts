// Solid 1.x declarations for the eslint-plugin-solid parity corpus.
// Verified against solid-js 1.9.14; see docs/solid-1x-api-surface.md.
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => void;
  export type Component<P = {}> = (props: P) => unknown;

  export function For<T>(props: {
    each: readonly T[];
    children: (item: T, index: Accessor<number>) => unknown;
  }): unknown;
  export function Index<T>(props: {
    each: readonly T[];
    children: (item: Accessor<T>, index: number) => unknown;
  }): unknown;
  export function Match<T>(props: {
    when: T;
    children: ((value: Accessor<NonNullable<T>>) => unknown) | unknown;
  }): unknown;
  export function Switch(props: { children: unknown }): unknown;
  export function Show<T>(props: {
    when: T;
    children: (value: Accessor<NonNullable<T>>) => unknown;
  }): unknown;
  export function Show<T>(props: {
    when: T;
    keyed: true;
    children: (value: NonNullable<T>) => unknown;
  }): unknown;
  export function createContext<T>(value: T): unknown;
  export function createEffect<T>(
    compute: (previous?: T) => T,
    value?: T,
    options?: { name?: string; render?: boolean },
  ): void;
  export function createRenderEffect<T>(
    compute: (previous?: T) => T,
    value?: T,
  ): void;
  export function createComputed<T>(compute: (previous?: T) => T, value?: T): void;
  export function createMemo<T>(compute: (previous?: T) => T, value?: T): Accessor<T>;
  export function createRoot<T>(callback: (dispose: () => void) => T): T;
  export function createSignal<T>(
    value: T,
    options?: { name?: string; equals?: false | ((previous: T, next: T) => boolean) },
  ): [Accessor<T>, Setter<T>];
  export function createResource<T>(
    fetcher: () => T | Promise<T>,
  ): [Accessor<T | undefined> & { loading: boolean }, { refetch: () => void }];
  export function mapArray<T, U>(
    items: Accessor<T[]>,
    map: (item: T) => U,
  ): Accessor<U[]>;
  export function onCleanup(callback: () => void): void;
  export function onMount(callback: () => void): void;
  export function batch<T>(callback: () => T): T;
  export function untrack<T>(callback: () => T): T;
  export function mergeProps<T, U>(defaults: T, props: U): T & U;
  export function splitProps<T extends object, K extends keyof T>(
    props: T,
    keys: K[],
  ): [Pick<T, K>, Omit<T, K>];
}

declare module "solid-js/store" {
  export function createStore<T extends object>(
    value: T,
  ): [T, (update: (draft: T) => void) => void];
  export function produce<T>(recipe: (draft: T) => void): (value: T) => T;
  export function reconcile<T>(value: T, options?: { key?: string }): (previous: T) => T;
  export function unwrap<T>(value: T): T;
}

declare module "solid-js/web" {
  export function Dynamic<T>(props: { component: T; [key: string]: unknown }): unknown;
  export function render(code: () => unknown, element: unknown): () => void;
}

declare namespace JSX {
  interface Element {}
  interface IntrinsicElements {
    button: Record<string, unknown>;
    div: Record<string, unknown>;
    h1: Record<string, unknown>;
    span: Record<string, unknown>;
  }
}
