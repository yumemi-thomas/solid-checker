declare module "solid-js" {
  type ComputeFunction<Prev, Next extends Prev = Prev> = (value: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  type EffectFunction<Prev, Next extends Prev = Prev> = (value: Next, previous?: Prev) => (() => void) | void;
  export function createEffect<T>(compute: ComputeFunction<undefined | T, T>, effect: EffectFunction<T, T>): void;
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function onCleanup(fn: () => void): void;
  export function untrack<T>(fn: () => T): T;
}

declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: unknown };
    i: {};
  }
  interface Element {}
}
