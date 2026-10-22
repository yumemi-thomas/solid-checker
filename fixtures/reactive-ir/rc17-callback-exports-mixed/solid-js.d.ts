// Declarations transcribed from solid-js@2.0.0-rc.17 and @solidjs/web@2.0.0-rc.17,
// as `rc17-dynamic-component`, `rc17-dispatch-as-interaction`,
// `rc17-create-deferred`, `rc17-lazy-module` and `rc17-on-hydration-end`
// transcribe them (the same lines, the same reductions).

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
  export const createDeferred: {
    <T>(compute: (v: undefined | NoInfer<T>) => PromiseLike<T> | AsyncIterable<T> | T, options?: object): Accessor<T>;
  };
  export function lazyModule<T extends Record<string, any>>(fn: () => Promise<T>, moduleUrl?: string): (() => Promise<T>) & {
      preload: () => Promise<T>;
      peek: () => T | undefined;
      moduleUrl?: string;
  };
  export function onHydrationEnd(callback: () => void): void;
  export const dynamicComponent: {
      <C extends Component<any>>(source: () => C | Promise<C> | AsyncIterable<C> | null | undefined | false, options?: DynamicOptions): Component<ComponentProps<C>>;
  };
}

declare module "@solidjs/web" {
  export function dispatchAsInteraction<T>(e: Event, fn: () => T): T;
}
