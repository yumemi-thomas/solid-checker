declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
}
