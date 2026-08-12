// The core module deliberately declares createStore even though Solid 1.x
// exports it only from solid-js/store: the wrong-subpath import must resolve
// at the type level so the checker's per-subpath export index — not
// TypeScript resolution — is what reports it (SC8002, v1/imports).
declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createStore<T extends object>(
    value: T,
  ): [T, (update: (draft: T) => void) => void];
}
declare module "solid-js/store" {
  export function createStore<T extends object>(
    value: T,
  ): [T, (update: (draft: T) => void) => void];
}
