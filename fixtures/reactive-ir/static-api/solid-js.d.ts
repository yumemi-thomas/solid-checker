declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

declare module "solid-js" {
  type Options = { sync?: boolean };
  export function createEffect<T>(compute: () => T, effect?: ((value: T) => void), options?: Options): void;
  export function createMemo<T>(compute: () => T, options?: Options): () => Awaited<T>;
  export function createSignal<T>(compute: () => T, options?: Options): [() => Awaited<T>, (value: Awaited<T>) => void];
  export function createStore<T extends object>(compute: () => T, seed: Awaited<T>, options?: Options): [Awaited<T>, (value: Awaited<T>) => void];
  export function createProjection<T extends object>(compute: () => T, seed: Awaited<T>, options?: Options): Awaited<T>;
  export function createOptimistic<T>(compute: () => T, options?: Options): [() => Awaited<T>, (value: Awaited<T>) => void];
  export function createOptimisticStore<T extends object>(compute: () => T, seed: Awaited<T>, options?: Options): [Awaited<T>, (value: Awaited<T>) => void];
  export function refresh(target: unknown): void;
  export function affects(...target: unknown[]): void;
}
