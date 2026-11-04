declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function createStore<T extends object>(value: T, options?: { shallow?: boolean; name?: string }): [T, (fn: (draft: T) => void) => void];
}
