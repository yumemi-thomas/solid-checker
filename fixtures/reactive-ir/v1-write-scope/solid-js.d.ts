declare module "solid-js" {
  export type Component = () => unknown;
  export type Setter<T> = (value: T | ((previous: T) => T)) => T;
  export function createSignal<T>(value: T): [() => T, Setter<T>];
  export function createMemo<T>(compute: () => T): () => T;
  export function createEffect(compute: () => void): void;
  export function createRenderEffect(compute: () => void): void;
  export function onMount(callback: () => void): void;
}
