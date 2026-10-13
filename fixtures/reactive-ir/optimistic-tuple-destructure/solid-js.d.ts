declare namespace JSX {
  interface IntrinsicElements { span: { children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  type ComputeFunction<Prev, Next extends Prev = Prev> = (value: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  type EffectFunction<Prev, Next extends Prev = Prev> = (value: Next, previous?: Prev) => (() => void) | void;
  type EffectBundle<Prev, Next extends Prev = Prev> = { effect: EffectFunction<Prev, Next>; error: (error: unknown, cleanup: () => void) => void };
  export function createEffect<T>(compute: ComputeFunction<undefined | T, T>, effect: EffectFunction<T, T> | EffectBundle<T, T>, options?: { name?: string }): void;
  /** @deprecated The client runtime throws MISSING_EFFECT_FN. */
  export function createEffect<T>(compute: ComputeFunction<undefined | T, T>): never;

  // The two under test. Both return a two-slot tuple, which is the shape the
  // claim depends on; see README.md for what is reduced and why it cannot
  // manufacture a finding.
  export function createOptimistic<T>(value: Exclude<T, Function>, options?: { name?: string }): [get: () => T, set: (next: T) => T];
  export function createOptimisticStore<T extends object>(store: T, options?: { name?: string; shallow?: boolean }): [get: T, set: (next: Partial<T>) => void];

  export function createStore<T extends object>(store: T, options?: { name?: string }): [get: T, set: (next: Partial<T>) => void];

  // The control: a source the hardcoded list always recognized.
  export function createSignal<T>(value: Exclude<T, Function>, options?: { name?: string }): [get: () => T, set: (next: T) => T];
}
