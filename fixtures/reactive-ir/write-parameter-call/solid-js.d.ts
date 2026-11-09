// Declarations transcribed from @solidjs/signals@2.0.0-rc.21, which solid-js@2.0.0-rc.21 re-exports,
// reduced to the overloads this fixture calls: `Accessor`, `Setter` (`dist/types/signals.d.ts:55`,
// `:66`), `createSignal` (`:302`), `createMemo` (`:358`, options dropped), `action`
// (`core/action.d.ts:78`). `JSX` is this file's, reduced to the element the fixture renders.
declare namespace JSX {
  interface IntrinsicElements {
    i: { children?: any; onClick?: (event: MouseEvent) => void };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<in out T> = {
    <U extends T>(...args: undefined extends T ? [] : [value: Exclude<U, Function> | ((prev: T) => U)]): undefined extends T ? undefined : U;
    <U extends T>(value: (prev: T) => U): U;
    <U extends T>(value: Exclude<U, Function>): U;
    <U extends T>(value: Exclude<U, Function> | ((prev: T) => U)): U;
  };
  export function createSignal<T>(value: Exclude<T, Function>): [get: Accessor<T>, set: Setter<T>];
  export function createMemo<T>(compute: (v: undefined | NoInfer<T>) => PromiseLike<T> | AsyncIterable<T> | T): Accessor<T>;
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
}
