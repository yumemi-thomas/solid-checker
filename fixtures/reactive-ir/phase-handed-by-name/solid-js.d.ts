// Declarations reduced from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17
// to the overloads this fixture calls, with no claim reading them back:
// `createSignal` to its value overload and `createMemo` to its plain overload
// (`dist/types/signals.d.ts:302`, `:358`), `createEffect` to a compute and an
// effect function (`:427`), `createTrackedEffect` (`:503`) and `onSettled`
// (`:737`); `flush` (`dist/types/core/scheduler.d.ts:384-385`) and `action`
// (`dist/types/core/action.d.ts:70`) byte-faithful. The standard library
// (`Array.forEach`, `Reflect.apply`, `setTimeout`, `addEventListener`) is
// TypeScript's own lib.

declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: any; onClick?: (event: MouseEvent) => void };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createMemo<T>(compute: (prev: T | undefined) => T): Accessor<T>;
  export function createEffect<const T>(compute: (prev: T | undefined) => T, effect: (value: T, prev?: T) => void | (() => void)): void;
  export function createTrackedEffect(compute: () => void | (() => void)): void;
  export function onSettled(callback: () => void | (() => void)): void;
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
}
