// Declarations transcribed from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17, reduced to the overloads this fixture calls:
// `createSignal` (`dist/types/signals.d.ts:302`), `untrack` (`dist/types/core/core.d.ts:167`), and `createStore` to its plain form with
// `StoreSetter` (`store/store.d.ts:21`). No claim here reads a return type.
declare namespace JSX {
  interface IntrinsicElements {
    p: { ref?: (element: HTMLParagraphElement) => void; children?: any };
  }
  interface Element {}
  interface ElementChildrenAttribute {
    children: {};
  }
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export type StoreSetter<T> = (fn: (state: T) => T | void) => void;
  export function createStore<T extends object>(value: T): [get: T, set: StoreSetter<T>];
  export function untrack<T>(fn: () => T): T;
}
