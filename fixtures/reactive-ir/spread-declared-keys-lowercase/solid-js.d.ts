declare namespace JSX {
  interface IntrinsicElements {
    div: { [attribute: string]: unknown; children?: unknown };
  }
}

declare module "solid-js" {
  // The published shape (`@solidjs/signals` `omit` and its `Omit`).
  export type Omit<T, K extends readonly (keyof T)[]> = {
    [P in keyof T as Exclude<P, K[number]>]: T[P];
  };
  export function omit<T extends Record<any, any>, K extends readonly (keyof T)[]>(props: T, ...keys: K): Omit<T, K>;
}

declare module "@solidjs/web" { export import JSX = globalThis.JSX; }
