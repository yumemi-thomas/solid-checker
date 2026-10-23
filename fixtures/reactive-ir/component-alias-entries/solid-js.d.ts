declare namespace JSX {
  interface IntrinsicElements {
    b: {};
    i: {};
    p: {};
    button: { onClick?: () => void };
  }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createEffect<T>(
    compute: () => T,
    apply: (value: T, previous?: T) => (() => void) | void,
  ): void;
}

declare module "@solidjs/web" { export import JSX = globalThis.JSX; }
