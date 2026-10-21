declare namespace JSX {
  type RefCallback<T> = (el: T) => void;
  type Ref<T> = T | RefCallback<T> | undefined | Ref<T>[];
  interface IntrinsicElements { div: { ref?: Ref<HTMLDivElement>; children?: unknown } }
}

// Signatures copied from @solidjs/signals@2.0.0-rc.13 dist/types (signals.d.ts,
// core/core.d.ts), for the overloads this fixture calls.
declare module "solid-js" {
  export type ComputeFunction<Prev, Next extends Prev = Prev> = (v: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  export type EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) | void;
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T>): void;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
}
