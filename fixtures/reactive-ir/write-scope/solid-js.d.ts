declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void };
    div: {};
  }
}

declare module "solid-js" {
  export function createSignal<T>(value: T, options?: { ownedWrite?: boolean }): [() => T, (value: T | ((previous: T) => T)) => void];
  export function createStore<T extends object>(value: T): [T, (update: (draft: T) => void) => void];
  export function createOptimistic<T>(value: T): [() => T, (value: T | ((previous: T) => T)) => void];
  export function createOptimisticStore<T extends object>(value: T): [T, (update: (draft: T) => void) => void];
  export function createMemo<T>(compute: () => T): () => T;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function onSettled(callback: () => void): void;
  export function createTrackedEffect(callback: () => void): void;
  export function untrack<T>(callback: () => T): T;
  export function action<T extends (...args: any[]) => any>(callback: T): (...args: Parameters<T>) => Promise<unknown>;
}
