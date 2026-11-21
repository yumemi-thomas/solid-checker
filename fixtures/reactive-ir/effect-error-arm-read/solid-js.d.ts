// Declarations reduced from solid-js@2.0.0-rc.21 and @solidjs/signals@2.0.0-rc.21 to the overloads this fixture calls, as
// `effect-error-arm-owner`'s: `createSignal`'s value overload, `EffectFunction` and `EffectBundle`, `createEffect`,
// `createReaction`, `createRoot`; plus `createStore`'s plain overload and `flush`.

declare namespace JSX {
  interface IntrinsicElements {
    button: { children?: any; onClick?: (event: MouseEvent) => void };
    i: { children?: any };
    div: { children?: any };
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
  export function createEffect<const T>(compute: (prev: T | undefined) => T, effectFn: EffectFunction<T, T> | EffectBundle<T, T>): void;
  export function createReaction(effectFn: EffectFunction<undefined> | EffectBundle<undefined>): (tracking: () => void) => void;
  export function createRoot<T>(init: (() => T) | ((dispose: () => void) => T)): T;
  export function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
  export function flush(): void;
}
