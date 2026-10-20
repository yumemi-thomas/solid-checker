declare namespace JSX {
  interface IntrinsicElements { div: {}; b: {}; i: {}; li: {}; ul: {}; u: {} }
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createSignal<T>(value: T): [Accessor<T>, (value: T) => T];
}
