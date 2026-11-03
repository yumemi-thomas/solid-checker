declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: unknown };
    div: { children?: unknown };
    button: { onClick?: () => void; children?: unknown };
  }
  type Element = unknown;
}

// Reduced from `@solidjs/signals@2.0.0-rc.17` `dist/types/signals.d.ts`
// (`resolve` :552, `until` :642; `UntilOptions` and `Truthy` simplified).
declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => T];
  export function createMemo<T>(fn: () => T): () => T;
  export function untrack<T>(fn: () => T): T;
  export function resolve<T>(fn: () => T): Promise<T>;
  export function until<T>(fn: () => T, options?: { timeout?: number }): Promise<T>;
}
