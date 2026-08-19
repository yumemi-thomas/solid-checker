declare namespace JSX {
  interface IntrinsicElements { div: { ref?: unknown; children?: unknown }; section: {} }
  interface Element {}
}

declare module "solid-js" {
  type EffectFunction<Prev, Next extends Prev = Prev> = (value: Prev) => Next;
  export function createEffect<Next>(fn: EffectFunction<undefined | Next, Next>): void;
  export function createEffect<Next, Init = Next>(fn: EffectFunction<Init | Next, Next>, value: Init, options?: { name?: string; render?: boolean }): void;
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
