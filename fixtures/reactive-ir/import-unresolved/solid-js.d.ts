declare namespace JSX {
  interface IntrinsicElements { button: { onClick?: unknown; children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  // As loose as fixtures/reactive-ir/store-delete's: the claim here rests on
  // no Solid declaration, only on the import the compiler cannot resolve.
  export function createEffect<T>(compute: () => T, effect: (value: T) => void): void;
}
