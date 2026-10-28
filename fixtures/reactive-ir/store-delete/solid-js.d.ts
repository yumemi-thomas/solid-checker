declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  // `Store<T> = T` on @solidjs/signals rc.7 and later (the audited rc.17 too).
  // The tsc oracle case `no-store-mutation-outside-setter` (store delete)
  // checks the delete against the published typings; this stub is as loose.
  export function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
  export function createProjection<T extends object>(fn: (draft: T) => void, seed?: T): T;
  export function createEffect<T>(compute: () => T, effect: (value: T) => void): void;
}
