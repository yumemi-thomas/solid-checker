declare module "solid-js" {
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function onCleanup(callback: () => void): void;
}
