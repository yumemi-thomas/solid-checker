declare namespace JSX {
  type EventHandler<T, E> = (event: E & { currentTarget: T; target: Element }) => void;
  interface IntrinsicElements {
    button: { onPointerDown?: EventHandler<HTMLButtonElement, PointerEvent> | [(data: any, event: PointerEvent) => void, any]; children?: unknown; [attribute: string]: unknown };
  }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createRenderEffect<T>(compute: () => T, apply: (value: T) => void): void;
}

declare module "@solidjs/web" { export import JSX = globalThis.JSX; }
