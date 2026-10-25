declare namespace JSX {
  type EventHandler<T, E> = (event: E & { currentTarget: T; target: Element }) => void;
  interface IntrinsicElements {
    button: { onClick?: EventHandler<HTMLButtonElement, MouseEvent>; children?: unknown };
    div: { children?: unknown };
  }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createRenderEffect<T>(compute: () => T, apply: (value: T) => void): void;
  export function createRoot<T>(fn: (dispose: () => void) => T): T;
}

declare module "@solidjs/web" { export import JSX = globalThis.JSX; }
