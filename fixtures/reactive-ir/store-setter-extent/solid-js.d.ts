declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
  // `Store<T> = T` on @solidjs/signals rc.7 and later (the audited rc.17 too),
  // so a write through the read proxy type-checks. The tsc oracle case
  // `no-store-mutation-outside-setter` (array mutators) checks that against
  // the published typings; this stub only has to be as loose.
  export function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
  export function createStore<T extends object>(fn: (draft: T) => void, seed: T): [T, (fn: (draft: T) => void) => void];
  export function createProjection<T extends object>(fn: (draft: T) => void, seed?: T): T;
}
