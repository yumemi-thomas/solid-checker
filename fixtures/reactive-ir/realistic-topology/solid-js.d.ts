declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: any };
    h1: { children?: any };
    span: { children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createSignal<T>(value: T): [Accessor<T>, (next: T) => void];
  export function createMemo<T>(compute: () => T): Accessor<T>;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function onCleanup(fn: () => void): void;
  export function createRoot<T>(fn: (dispose: () => void) => T): T;
}
