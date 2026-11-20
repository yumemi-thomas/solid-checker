// The rc.21 declarations this fixture uses (`@solidjs/signals` 2.0.0-rc.21 dist/types/signals.d.ts), reduced to the overloads written here.
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T | ((prev: T) => T)) => T;
  export function createSignal<T>(value: T): [Accessor<T>, Setter<T>];
  export function createRenderEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
}
