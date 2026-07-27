declare module "solid-js" {
  export function createStore<T extends object>(value: T): [T, (update: Partial<T>) => void];
}

declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
  }
}
