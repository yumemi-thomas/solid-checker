declare namespace JSX {
  interface IntrinsicElements { div: {}; button: { onClick?: () => void; children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function createRoot<T>(callback: () => T): T;
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(callback: () => T): () => T;
  export function onCleanup(callback: () => void): void;
  export function onSettled(callback: () => void | (() => void)): void;
  export function Loading(props: { fallback: JSX.Element; children: JSX.Element }): JSX.Element;
}
