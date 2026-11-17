declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
  interface ElementChildrenAttribute { children: {} }
}

declare module "solid-js" {
  // The published 2.0.0-rc.21 `@solidjs/signals` signatures reduced to the forms used here (`dist/types/signals.d.ts`
  // `createMemo`, `createEffect` with `MemoOptions`/`EffectOptions` cut to `sync`; `dist/types/core/core.d.ts` `untrack`).
  export function createMemo<T>(compute: (prev: T | undefined) => T, options?: { sync?: boolean }): () => T;
  export function createEffect<T>(
    compute: (prev: T | undefined) => T,
    effect: (value: T, prev?: T) => (() => void) | void,
    options?: { sync?: boolean },
  ): void;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
}
