declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: () => void };
    div: {};
  }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
}
