declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: unknown };
    div: { children?: unknown };
    button: { onClick?: () => void; children?: unknown };
  }
  type Element = unknown;
}

declare module "solid-js" {
  export type Owner = object;
  export function createSignal<T>(v: T): [() => T, (n: T) => T];
  export function createMemo<T>(fn: () => T): () => T;
  export function createRoot<T>(fn: (dispose: () => void) => T): T;
  export function getOwner(): Owner | null;
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T | undefined;
  export function untrack<T>(fn: () => T): T;
  export function resolve<T>(fn: () => T): Promise<T>;
}
