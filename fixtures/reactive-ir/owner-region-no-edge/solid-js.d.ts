// Declarations reduced from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17
// to the overloads this fixture calls, with no claim reading them back:
// `createSignal` to its value overload (`dist/types/signals.d.ts`), `onCleanup`
// (`:47`), `createEffect` to its compute-and-apply overload (`:58`, the apply
// typed to the value the compute returns) and `createRoot` to its callback
// overload (`core/core.d.ts`).

declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: any };
    button: { onClick?: (event: MouseEvent) => void; children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function onCleanup(fn: () => void): void;
  export function createEffect<T>(compute: () => T, effectFn: (value: T) => void | (() => void)): void;
  export function createRoot<T>(init: (dispose: () => void) => T): T;
}
