declare namespace JSX {
  interface IntrinsicElements {
    a: Record<string, unknown>;
    div: Record<string, unknown>;
    input: Record<string, unknown>;
    label: Record<string, unknown>;
    span: Record<string, unknown>;
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
}
