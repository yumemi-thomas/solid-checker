declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
}

declare namespace JSX {
  interface IntrinsicElements {
    button: Record<string, unknown>;
    div: Record<string, unknown>;
  }
}
