// The effect constructors as @solidjs/signals@2.0.0-rc.21 declares them (dist/types/signals.d.ts), options reduced to the keys
// this fixture never passes.
declare module "solid-js" {
  type ComputeFunction<Prev, Next extends Prev = Prev> = (v: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
  type EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) | void;
  type EffectBundle<Prev, Next extends Prev = Prev> = {
    effect: EffectFunction<Prev, Next>;
    error: (err: unknown, cleanup: () => void) => void;
  };
  interface EffectOptions { name?: string; defer?: boolean }
  export function createEffect<const T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T> | EffectBundle<NoInfer<T>, T>, options?: EffectOptions): void;
  export function createRenderEffect<const T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T>, options?: EffectOptions): void;
  export function createReaction(effectFn: EffectFunction<undefined> | EffectBundle<undefined>, options?: EffectOptions): (tracking: () => void) => void;
}
