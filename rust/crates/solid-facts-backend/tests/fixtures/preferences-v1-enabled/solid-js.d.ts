declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => T];
  export function createMemo<T>(compute: () => T, value?: T, options?: object): Accessor<T>;
  export function untrack<T>(compute: () => T): T;
}

declare module "unknown-reactivity" {
  export function externalList(): string[];
  export function externalReady(): boolean;
}

declare module "solid-js/store" {
  export function createStore<T extends object>(value: T): [T, (...args: unknown[]) => void];
}

declare namespace JSX {
  interface IntrinsicElements {
    main: { children?: unknown };
    section: { children?: unknown };
    ul: { children?: unknown };
    span: { children?: unknown; title?: string };
  }
  interface Element {}
}
