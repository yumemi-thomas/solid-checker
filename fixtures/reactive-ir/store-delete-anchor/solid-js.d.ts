declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  // `Store<T> = T` on @solidjs/signals rc.7 and later (rc.21 too), as in
  // fixtures/reactive-ir/store-delete; this stub is as loose.
  export function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void];
}
