declare module "solid-js" {
  export type Component<P = {}> = (props: P) => unknown;
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function onCleanup(callback: () => void): void;
}
