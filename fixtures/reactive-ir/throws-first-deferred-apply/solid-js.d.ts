declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: unknown };
    button: { onClick?: () => void; children?: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  // Reduced from @solidjs/signals rc.21 (`dist/types/signals.d.ts`, `core/owner.d.ts`, `core/scheduler.d.ts`): `createEffect` with
  // `EffectOptions.defer`, `createMemo` with `MemoOptions.sync`, `createSignal`'s value form, `createRoot`, `flush()`.
  export function createEffect<T>(compute: (v: T | undefined) => T, effectFn: (v: T, p?: T) => (() => void) | void, options?: { defer?: boolean }): void;
  export function createMemo<T>(compute: () => T, options?: { sync?: boolean }): () => T;
  export function createSignal<T>(value: T): [() => T, (value: T) => T];
  export function createRoot<T>(init: () => T): T;
  export function flush(): void;
}
