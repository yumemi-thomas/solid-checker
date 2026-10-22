// Declarations transcribed from solid-js@2.0.0-rc.17 and @solidjs/web@2.0.0-rc.17,
// as `rc17-dynamic-component` and `rc17-dispatch-as-interaction` transcribe
// them (the same lines, the same reductions).

declare namespace JSX {
  interface IntrinsicElements {}
  interface Element {}
}

declare module "solid-js" {
  type SolidElement = JSX.Element | null;
  export type Component<P extends Record<string, any> = {}> = (props: P) => SolidElement;
  export type ComponentProps<T extends Component<any>> = T extends Component<infer P> ? P : never;
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createEffect<Next, Init = Next>(
    compute: (prev: Init | Next) => Next,
    effect: (value: Next, prev?: Next) => (() => void) | void,
    options?: object
  ): void;
  export interface DynamicOptions {
      deferStream?: boolean;
      static?: boolean;
  }
  export const dynamicComponent: {
      <C extends Component<any>>(source: () => C | Promise<C> | AsyncIterable<C> | null | undefined | false, options?: DynamicOptions): Component<ComponentProps<C>>;
  };
}

declare module "@solidjs/web" {
  export function dispatchAsInteraction<T>(e: Event, fn: () => T): T;
}
