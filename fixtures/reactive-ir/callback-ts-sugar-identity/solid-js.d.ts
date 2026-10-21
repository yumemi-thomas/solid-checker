declare module "solid-js" {
  type ComputeFunction<Prev, Next extends Prev = Prev> = (value: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  type EffectFunction<Prev, Next extends Prev = Prev> = (value: Next, previous?: Prev) => (() => void) | void;
  export function createEffect<T>(compute: ComputeFunction<undefined | T, T>, effect: EffectFunction<T, T>): void;
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function createReaction(fn: () => void): (tracking: () => void) => void;
  export function onCleanup(fn: () => void): void;
}

declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: unknown; ref?: unknown };
    div: { ref?: unknown; children?: unknown };
  }
  interface Element {}
}
