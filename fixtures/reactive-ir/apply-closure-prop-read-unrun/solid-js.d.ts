declare namespace JSX {
  interface IntrinsicElements { div: {}; p: {} }
  interface Element {}
}

declare module "solid-js" {
  // Reduced from @solidjs/signals rc.21 (`dist/types/core/*.d.ts`): `createSignal`'s value form and `createEffect`.
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [Accessor<T>, Setter<T>];
  export function createEffect<T>(compute: () => T, apply: (value: T) => void): void;
}

declare module "@solidjs/web" {
  export function render(code: () => JSX.Element, element: Element): () => void;
}
