// Declarations transcribed from solid-js@2.0.0-rc.17 and
// @solidjs/signals@2.0.0-rc.17.
//
// Byte-faithful where a claim reads them: `createRoot`
// (`dist/types/core/owner.d.ts:139`), up to the `export declare` -> `export`
// an ambient block forbids. Reduced, with no claim reading them back:
// `Owner` and `Root` to the interfaces' names (`dist/types/core/types.d.ts:153`,
// `:208`) and `createSignal` to its value overload.

declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: (event: MouseEvent) => void; children?: any };
  }
  interface Element {}
  interface ElementChildrenAttribute { children: {} }
}

declare module "solid-js" {
  export function For<T>(props: { each: readonly T[]; children: (item: T, index: () => number) => JSX.Element }): JSX.Element;
  export interface Owner {}
  export interface Root extends Owner {}
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createRoot<T>(
    init: ((dispose: () => void) => T) | (() => T),
    options?: { id?: string; transparent?: boolean }
  ): T;
}
