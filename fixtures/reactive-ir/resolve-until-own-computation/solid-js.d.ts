declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: unknown; children?: unknown }; div: { children?: unknown }; p: { children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  type Teardown = (() => void) | { unsubscribe(): void };
  export type FromSource<T> = (next: (value: T) => void, error: (err: unknown) => void, complete: () => void) => Teardown;
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => void];
  export function createMemo<T>(fn: () => T | Promise<T> | AsyncIterable<T>, options?: { loadingValue?: T }): Accessor<T>;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function untrack<T>(fn: () => T, label?: string | false): T;
  export function onCleanup(fn: () => void): () => void;
  export function resolve<T>(fn: () => T): Promise<T>;
  export function until<T>(fn: () => T): Promise<T>;
  export function from<T>(source: FromSource<T>): AsyncIterable<T>;
}
