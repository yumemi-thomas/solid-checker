declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void; children?: unknown };
    i: {};
    div: { children?: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  // Reduced from @solidjs/signals rc.21: `Store<T> = T`, the plain `createStore` overload and `StoreSetter`.
  export type StoreSetter<T> = (fn: (draft: T) => void) => void;
  export function createStore<T extends object>(value: T): [T, StoreSetter<T>];
}
