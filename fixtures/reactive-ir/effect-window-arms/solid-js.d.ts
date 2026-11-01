// Declarations reduced from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17
// to the overloads this fixture calls: `createSignal` to its value overload
// (`dist/types/signals.d.ts:302`), `createMemo` to its plain overload (`:358`),
// `EffectFunction` and `EffectBundle` (`:75-90`), `createEffect` (`:427`) and
// `createReaction` (`:530`); `action` (`dist/types/core/action.d.ts:70`).

declare namespace JSX {
  interface IntrinsicElements {
    button: { children?: any; onClick?: (event: MouseEvent) => void };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export type EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) | void;
  export type EffectBundle<Prev, Next extends Prev = Prev> = {
    effect: EffectFunction<Prev, Next>;
    error: (err: unknown, cleanup: () => void) => void;
  };
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createMemo<T>(compute: (prev: T | undefined) => T): Accessor<T>;
  export function createEffect<const T>(compute: (prev: T | undefined) => T, effectFn: EffectFunction<T, T> | EffectBundle<T, T>): void;
  export function createReaction(effectFn: EffectFunction<undefined> | EffectBundle<undefined>): (tracking: () => void) => void;
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
}
