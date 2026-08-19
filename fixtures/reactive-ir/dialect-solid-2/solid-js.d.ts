declare namespace JSX {
  interface IntrinsicElements { div: { ref?: unknown; children?: unknown }; section: {} }
  interface Element {}
}

declare module "solid-js" {
  type ComputeFunction<Prev, Next extends Prev = Prev> = (value: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  type EffectFunction<Prev, Next extends Prev = Prev> = (value: Next, previous?: Prev) => (() => void) | void;
  type EffectBundle<Prev, Next extends Prev = Prev> = { effect: EffectFunction<Prev, Next>; error: (error: unknown, cleanup: () => void) => void };
  export function createEffect<T>(compute: ComputeFunction<undefined | T, T>, effect: EffectFunction<T, T> | EffectBundle<T, T>, options?: { name?: string }): void;
  /** @deprecated The client runtime throws MISSING_EFFECT_FN. */
  export function createEffect<T>(compute: ComputeFunction<undefined | T, T>): never;
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function batch<T>(fn: () => T): T;
  export function onMount(fn: () => void): void;
  export function createReaction(fn: () => void): (tracking: () => void) => void;
  export function onCleanup(fn: () => void): void;
  export function createMemo<T>(fn: () => T, options?: { sync?: boolean }): () => T;
  export function createMemo<T>(fn: () => T, value: T | undefined, options?: { sync?: boolean }): () => T;
}

declare module "solid-js/store" {
  export function createStore<T extends object>(value: T): [T, (next: Partial<T>) => void];
}
