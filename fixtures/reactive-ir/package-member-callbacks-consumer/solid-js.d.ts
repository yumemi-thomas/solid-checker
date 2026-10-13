declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
}
