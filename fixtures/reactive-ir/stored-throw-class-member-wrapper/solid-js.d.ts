// Stubs of the published 2.0.0-rc.21 signatures this fixture uses:
// `createMemo`, `createEffect` and `flush` (`@solidjs/signals` `dist/types/core/scheduler.d.ts:448-449`),
// and `@solidjs/web`'s `dynamic` (`types/index.d.ts`), whose source is typed
// `() => T | Promise<T> | ...` whatever `static` says.
declare namespace JSX {
  interface IntrinsicElements { div: {}; button: { onClick?: () => void } }
  interface Element {}
  interface ElementChildrenAttribute { children: {} }
}

declare module "solid-js" {
  export function createMemo<T>(fn: () => T): () => T;
  export function createEffect<T>(compute: () => T, effect: (value: T) => void): void;
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
  export interface DynamicOptions { static?: boolean }
}

declare module "@solidjs/web" {
  import type { DynamicOptions } from "solid-js";
  export function dynamic<T>(source: () => T | Promise<T> | null | undefined | false, options?: DynamicOptions): () => unknown;
}
