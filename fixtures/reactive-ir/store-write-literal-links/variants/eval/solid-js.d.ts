declare namespace JSX {
  interface IntrinsicElements { p: { children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  // `Store<T> = T` on @solidjs/signals rc.7 and later (the audited rc.17 too),
  // so a write through the read proxy type-checks.
  export function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
}
