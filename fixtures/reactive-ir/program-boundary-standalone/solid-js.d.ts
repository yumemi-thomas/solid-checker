declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void };
  }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
}

declare module "@solidjs/web" { export import JSX = globalThis.JSX; }
