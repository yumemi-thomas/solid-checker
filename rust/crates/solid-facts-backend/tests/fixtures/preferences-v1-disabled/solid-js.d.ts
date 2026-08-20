declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => T];
}

declare namespace JSX {
  interface IntrinsicElements {
    main: { children?: unknown };
    span: { children?: unknown };
  }
  interface Element {}
}
