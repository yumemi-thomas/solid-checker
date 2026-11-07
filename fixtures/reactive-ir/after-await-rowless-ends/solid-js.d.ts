// Declarations transcribed from solid-js@2.0.0-rc.21 and @solidjs/signals@2.0.0-rc.21, reduced to what this fixture calls: `createSignal` and
// `createStore` to their plain forms, `createMemo` to a compute that may return a promise (`ComputeFunction`), `createEffect` to its
// two-function shape whose apply returns `(() => void) | void`, as the package says. No claim here reads a return type.
declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: any };
  }
  interface Element {}
  interface ElementChildrenAttribute {
    children: {};
  }
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: (value: T) => T];
  export function createMemo<T>(compute: (prev: T | undefined) => T | PromiseLike<T>): Accessor<T>;
  export function createEffect<Next, Init = Next>(
    compute: (prev: Init | Next) => Next | Promise<Next>,
    effect: (value: Next, prev?: Next) => (() => void) | void,
    options?: object
  ): void;
  export type StoreSetter<T> = (fn: (state: T) => T | void) => void;
  export function createStore<T extends object>(value: T): [get: T, set: StoreSetter<T>];
}
