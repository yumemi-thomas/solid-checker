declare namespace JSX {
  interface IntrinsicElements { div: {}; p: {}; button: { onClick?: () => void } }
  interface Element {}
}

declare module "solid-js" {
  // Reduced from @solidjs/signals rc.21 (`dist/types/core/*.d.ts`): `createSignal`'s value form and `createRenderEffect`.
  export function createSignal<T>(value: T): [() => T, (value: T) => T];
  export function createRenderEffect<T>(compute: () => T, apply: (value: T) => void, options?: { defer?: boolean }): void;
}

declare module "@solidjs/web" {
  export function render(code: () => JSX.Element, element: Element): () => void;
}
