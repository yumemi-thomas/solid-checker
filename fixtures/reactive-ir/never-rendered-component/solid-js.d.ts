declare namespace JSX {
  interface IntrinsicElements {
    div: { onClick?: (event: MouseEvent) => void; class?: string; children?: unknown; [attribute: string]: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function omit<T extends object, K extends readonly (keyof T)[]>(props: T, ...keys: K): Omit<T, K[number]>;
}

declare module "@solidjs/web" { export import JSX = globalThis.JSX; }
