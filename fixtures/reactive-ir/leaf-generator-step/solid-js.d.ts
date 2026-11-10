// Declarations transcribed from @solidjs/signals@2.0.0-rc.21, which solid-js@2.0.0-rc.21 re-exports,
// reduced to the overloads this fixture calls: `onCleanup` (`dist/types/signals.d.ts:47`, `Disposable` `core/types.d.ts:3`),
// `createTrackedEffect` (`:503`), `onSettled` (`:737`), `createRoot` (`core/owner.d.ts:139`), `flush`
// (`core/scheduler.d.ts:448`). `JSX` is this file's, reduced to the element the fixture renders.
declare namespace JSX {
  interface IntrinsicElements { i: {} }
  interface Element {}
}

declare module "solid-js" {
  export interface Disposable { (): void }
  export function onCleanup(fn: Disposable): Disposable;
  export function createTrackedEffect(compute: () => void | (() => void)): void;
  export function onSettled(callback: () => void | (() => void)): void;
  export function createRoot<T>(init: ((dispose: () => void) => T) | (() => T)): T;
  export function flush(): void;
}
