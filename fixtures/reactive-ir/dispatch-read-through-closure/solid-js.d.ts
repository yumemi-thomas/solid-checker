declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: unknown };
    button: { onClick?: () => void; children?: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => T];
}
