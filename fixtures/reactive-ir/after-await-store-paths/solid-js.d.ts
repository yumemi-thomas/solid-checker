// Declarations transcribed from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17, reduced to what this fixture calls: `createEffect` to its
// two-function shape (`dist/types/signals.d.ts:427`) whose apply returns `(() => void) | void`, as the package says, and `createStore` to its plain
// form with `StoreSetter` (`store/store.d.ts:21`). No claim here reads a return type.
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
  export function createEffect<Next, Init = Next>(
    compute: (prev: Init | Next) => Next | Promise<Next>,
    effect: (value: Next, prev?: Next) => (() => void) | void,
    options?: object
  ): void;
  export type StoreSetter<T> = (fn: (state: T) => T | void) => void;
  export function createStore<T extends object>(value: T): [get: T, set: StoreSetter<T>];
}
