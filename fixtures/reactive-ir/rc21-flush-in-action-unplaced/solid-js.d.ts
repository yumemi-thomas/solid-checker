// Declarations transcribed from @solidjs/signals@2.0.0-rc.21, which solid-js@2.0.0-rc.21 re-exports:
// `flush` `dist/types/core/scheduler.d.ts:448-449`, `action` `dist/types/core/action.d.ts:78`.
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
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
}
