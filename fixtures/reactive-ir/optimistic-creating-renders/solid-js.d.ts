declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: unknown };
    i: { children?: unknown };
    button: { onClick?: () => void; children?: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  // Reduced from @solidjs/signals rc.21 (`dist/types/core/*.d.ts`): the function form of `createOptimistic`, `createMemo`
  // with its `lazy` option, `flush(fn)` and `createSignal`'s value form.
  export function createOptimistic<T>(fn: () => T | Promise<T>): [() => T, (value: T) => void];
  export function createMemo<T>(compute: () => T, options?: { lazy?: boolean }): () => T;
  export function flush<T>(fn: () => T): T;
  export function createSignal<T>(value: T): [() => T, (value: T) => T];
}
