declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  export function createReaction(fn: () => void): (tracking: () => void) => void;
  export function onCleanup(fn: () => void): void;
}
