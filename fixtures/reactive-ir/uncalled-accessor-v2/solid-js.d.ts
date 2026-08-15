declare namespace JSX {
  interface IntrinsicElements {
    div: { class?: unknown; title?: unknown; children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
}
