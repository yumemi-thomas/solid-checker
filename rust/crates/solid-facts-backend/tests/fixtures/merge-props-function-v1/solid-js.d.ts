declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
  }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function mergeProps<T extends unknown[]>(...sources: T): unknown;
}
