declare namespace JSX {
  interface IntrinsicElements { div: {} }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(compute: () => T): () => T;
  export function createRoot<T>(compute: () => T): T;
  export function createTrackedEffect(callback: () => unknown): void;
  export function createEffect<T>(compute: () => T, apply: (value: T) => unknown): void;
  export function onSettled(callback: () => unknown): void;
  export function onCleanup(callback: () => void): void;
  export function flush(): void;
}
