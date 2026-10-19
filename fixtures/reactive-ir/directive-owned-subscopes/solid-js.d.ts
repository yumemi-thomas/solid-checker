declare namespace JSX {
  interface IntrinsicElements { div: { ref?: unknown; children?: unknown } }
}

// Signatures copied from @solidjs/signals@2.0.0-rc.13 dist/types
// (core/owner.d.ts, core/core.d.ts); `Owner` is opaque here.
declare module "solid-js" {
  export interface Owner {}
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(compute: () => T): () => T;
  export function createRoot<T>(init: ((dispose: () => void) => T) | (() => T), options?: { id?: string; transparent?: boolean }): T;
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
  export function getOwner(): Owner | null;
}
