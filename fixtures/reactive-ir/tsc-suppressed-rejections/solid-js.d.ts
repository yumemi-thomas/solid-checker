declare module "solid-js" {
  // As strict as the published rc.21 typings for what this fixture calls:
  // `EffectFunction` returns `(() => void) | void`, there is no one-argument
  // `createEffect` overload, and `onSettled` takes `() => void | (() => void)`.
  export type Accessor<T> = () => T;
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => T];
  export function createEffect<T>(
    compute: () => T,
    effect: (value: T, previous?: T) => (() => void) | void,
    options?: { name?: string },
  ): void;
  export function onSettled(callback: () => void | (() => void)): void;
}
