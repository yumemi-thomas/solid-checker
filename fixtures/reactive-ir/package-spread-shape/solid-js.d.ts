declare namespace JSX {
  interface IntrinsicElements {
    div: { onClick?: (event: MouseEvent) => void; children?: unknown; [attribute: string]: unknown };
  }
}

declare module "solid-js" {
  export function onCleanup(fn: () => void): void;
}

declare module "@solidjs/web" { export import JSX = globalThis.JSX; }
