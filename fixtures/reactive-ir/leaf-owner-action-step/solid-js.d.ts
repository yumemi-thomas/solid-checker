// Declarations reduced from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17
// to the overloads this fixture calls: `createSignal` to its value overload
// (`dist/types/signals.d.ts:302`), `createMemo` to its plain overload (`:358`),
// `createTrackedEffect` (`:503`), `onSettled` (`:737`), `onCleanup` (`:47`,
// `Disposable` as a function), `createRoot` (`dist/types/core/owner.d.ts:139`),
// `action` (`dist/types/core/action.d.ts:70`).

declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createMemo<T>(compute: (prev: T | undefined) => T): Accessor<T>;
  export function createTrackedEffect(compute: () => void | (() => void)): void;
  export function onSettled(callback: () => void | (() => void)): void;
  export function onCleanup(fn: () => void): () => void;
  export function createRoot<T>(init: (() => T) | ((dispose: () => void) => T)): T;
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
}
