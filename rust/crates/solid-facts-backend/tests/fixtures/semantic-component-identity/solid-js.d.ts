declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => void;
  export type Component<P = {}> = (props: P) => unknown;
  export type ComponentProps<T> = T extends Component<infer P> ? P : never;

  export function createSignal<T>(value: T): [Accessor<T>, Setter<T>];
  export function createRoot<T>(callback: () => T): T;
}

declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: unknown };
    button: { onClick?: () => void };
  }
}
