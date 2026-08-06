declare namespace JSX {
  interface Element {}
  interface ElementChildrenAttribute { children: {} }
  interface IntrinsicElements { div: Record<string, unknown> }
}

declare module "solid-js" {
  export function batch<T>(fn: () => T): T;
  export function catchError<T>(fn: () => T, handler: (error: Error) => void): T | undefined;
  export function children<T>(fn: () => T): (() => T) & { toArray(): T[] };
  export interface Context<T> {
    Provider(props: { value: T; children?: JSX.Element }): JSX.Element;
  }

  export function createContext<T>(defaultValue: T): Context<T>;
  export function createEffect(fn: () => void): void;
  export function createSelector<T, U = T>(source: () => T, comparator?: (key: U, value: T) => boolean): (key: U) => boolean;
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function from<T>(producer: (setter: (value: T) => void) => () => void): () => T | undefined;
  export function lazy<T extends (props: object) => JSX.Element>(loader: () => Promise<{ default: T }>): T & { preload(): Promise<{ default: T }> };
  export function on<T, U>(dependency: () => T, fn: (value: T, previous: T | undefined, previousValue: U | undefined) => U): (previousValue?: U) => U;
  export function onCleanup(fn: () => void): void;
  export function useTransition(): [() => boolean, (fn: () => void) => void];
}

declare module "solid-js/store" {
  export function modifyMutable<T extends object>(state: T, modifier: (state: T) => void): void;
  export function produce<T extends object>(modifier: (state: T) => void): (state: T) => T;
}

declare module "solid-js/web" {
  export function createDynamic<T>(component: () => T, props: object): unknown;
  export function effect(fn: () => void): void;
  export function memo<T>(fn: () => T): () => T;
  export function hydrate(fn: () => unknown, element: Element): () => void;
  export function render(fn: () => unknown, element: Element): () => void;
}
