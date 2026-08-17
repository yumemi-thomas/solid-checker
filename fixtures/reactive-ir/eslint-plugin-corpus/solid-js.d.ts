declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => void;
  export type Component<P = {}> = (props: P) => unknown;

  export function For<T>(props: {
    each: readonly T[];
    children: (item: T, index: Accessor<number>) => unknown;
  }): unknown;
  export function For<T>(props: {
    each: readonly T[];
    keyed: false;
    children: (item: Accessor<T>, index: number) => unknown;
  }): unknown;
  export function For<T>(props: {
    each: readonly T[];
    keyed: (item: T) => unknown;
    children: (item: Accessor<T>, index: Accessor<number>) => unknown;
  }): unknown;
  export function Repeat(props: {
    count: number;
    children: (index: number) => unknown;
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
  export function action<T>(callback: (...args: never[]) => T): () => T;
  export function createOptimistic<T>(
    value: T,
    options?: { ownedWrite?: boolean },
  ): [Accessor<T>, Setter<T>];
  export function createContext<T>(value: T): unknown;
  // Byte-faithful to `@solidjs/signals@2.0.0-rc.0`'s
  // `EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) | void`
  // and `onSettled(callback: () => void | (() => void))`. Do not loosen these:
  // a stub that returns `unknown` manufactures cleanup-return defects no real
  // project can produce, which is how the removed SC3004/SC9002 pair survived
  // for a full cycle (docs/precision-backlog.md).
  export function createEffect<T>(
    compute: () => T,
    apply: (value: T, previous?: T) => (() => void) | void,
  ): void;
  export function createEffect<T>(
    compute: () => T,
    apply: {
      effect: (value: T, previous?: T) => (() => void) | void;
      error?: (error: unknown, cleanup: () => void) => (() => void) | void;
    },
  ): void;
  export function createMemo<T>(compute: () => T): Accessor<T>;
  export function createProjection<T>(
    compute: (draft: T) => void,
    seed: T,
  ): Accessor<T>;
  export function createRenderEffect<T>(
    compute: () => T,
    apply: (value: T, previous?: T) => (() => void) | void,
  ): void;
  export function createRoot<T>(callback: () => T): T;
  export function createSignal<T>(
    value: T,
    options?: { ownedWrite?: boolean },
  ): [Accessor<T>, Setter<T>];
  export function createSignal<T>(
    compute: () => T,
    options?: { ownedWrite?: boolean },
  ): [Accessor<T>, Setter<T>];
  export function createStore<T>(
    value: T,
  ): [T, (update: (draft: T) => void) => void];
  export function createStore<T>(
    compute: () => T,
    seed: T,
  ): [T, (update: (draft: T) => void) => void];
  // Byte-faithful to `@solidjs/signals@2.0.0-rc.0`'s
  // `createTrackedEffect(compute: () => void | (() => void), options?)`.
  export function createTrackedEffect(callback: () => void | (() => void)): void;
  export function flush<T = void>(callback?: () => T): T;
  export function mapArray<T, U>(
    items: Accessor<T[]>,
    map: (item: T) => U,
  ): Accessor<U[]>;
  export function onCleanup(callback: () => void): void;
  export function onSettled(callback: () => void | (() => void)): void;
  export function refresh(target: unknown): void;
  export function merge<T, U>(defaults: T, props: U): T & U;
  export function omit<T extends object, K extends keyof T>(
    props: T,
    ...keys: K[]
  ): Omit<T, K>;
  export function untrack<T>(callback: () => T): T;
}

declare module "@solidjs/web" {
  export function dynamic<T>(source: () => T): () => T;
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
