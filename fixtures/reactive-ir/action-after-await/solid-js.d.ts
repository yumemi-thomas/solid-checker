// The rc.21 declarations this fixture uses, as `@solidjs/signals` 2.0.0-rc.21 publishes them (dist/types/signals.d.ts,
// dist/types/core/action.d.ts:78), reduced to the overloads written here.
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T | ((prev: T) => T)) => T;
  export function createSignal<T>(value: T): [Accessor<T>, Setter<T>];
  export function action<Args extends any[], Y, R>(
    genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>,
  ): (...args: Args) => Promise<R>;
}
