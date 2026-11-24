declare namespace JSX {
  interface IntrinsicElements { div: {}; p: {}; button: { onClick?: () => void } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function onSettled(fn: () => void | (() => void)): void;
  export function createReaction(onInvalidate: () => void): (tracking: () => void) => void;
}

declare module "@solidjs/web" {
  export function render(code: () => JSX.Element, element: Element): () => void;
}
