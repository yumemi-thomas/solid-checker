declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

declare module "solid-js" {
  type Options = { sync?: boolean };
  export function createEffect<T>(compute: () => T, effect?: unknown, options?: Options): void;
  export function createMemo<T>(compute: () => T, options?: Options): () => Awaited<T>;
  export function createSignal<T>(compute: () => T, options?: Options): [() => Awaited<T>, (value: Awaited<T>) => void];
  export function createSignal<T>(value: T, options?: Options): [() => T, (value: T) => void];
  export function createStore<T extends object>(compute: () => T, seed: Awaited<T>, options?: Options): [Awaited<T>, (value: Awaited<T>) => void];
  export function createStore<T extends object>(value: T, options?: Options): [T, (value: T) => void];
  export function createProjection<T extends object>(compute: () => T, seed: Awaited<T>, options?: Options): Awaited<T>;
  export function createOptimistic<T>(compute: () => T, options?: Options): [() => Awaited<T>, (value: Awaited<T>) => void];
  export function createOptimisticStore<T extends object>(compute: () => T, seed: Awaited<T>, options?: Options): [Awaited<T>, (value: Awaited<T>) => void];
  // Deliberately looser than `@solidjs/signals@2.0.0-rc.0`, which brands the
  // target: `refresh<T>(target: Refreshable<T>)` and
  // `affects(target: Accessor<unknown> | Store<object>)` /
  // `affects<T extends object>(target: Store<T>, key: keyof T)`. The invalid
  // targets these fixtures write are TS2345 against the real signatures, which
  // is why the SC7003/SC7004/SC9003 family was removed. Nothing here proves a
  // finding from the looseness any more: what remains is the `refresh(...)`
  // *write*, whose targets are all valid. Do not add a rule that depends on it.
  export function refresh(target?: unknown, ...ignored: unknown[]): void;
  export function affects(target: unknown, key?: PropertyKey): void;
}
