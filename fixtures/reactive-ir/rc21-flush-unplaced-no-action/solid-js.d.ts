// Declarations transcribed from @solidjs/signals@2.0.0-rc.21, which solid-js@2.0.0-rc.21 re-exports:
// `flush` `dist/types/core/scheduler.d.ts:448-449` only: this fixture pins a project naming no stepping primitive.
// `JSX` is this file's, reduced to the one element the fixture renders.
declare namespace JSX {
  interface IntrinsicElements {
    button: { id?: string; onClick?: unknown; children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
}
