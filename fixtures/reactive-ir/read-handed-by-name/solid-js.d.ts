// Declarations reduced from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17
// to the overloads this fixture calls, with no claim reading them back:
// `createSignal` to its value overload (`dist/types/signals.d.ts`), `createMemo`
// to a plain compute, and `For` to the `each` and `children` its case passes
// (`types/server/flow.d.ts`). The standard library (`Array.forEach`, `Reflect.apply`,
// `JSON.parse`, `Function.call`, ...) is TypeScript's own lib.

declare namespace JSX {
  interface IntrinsicElements {
    li: { children?: any };
    p: { children?: any };
    button: { onClick?: (event: MouseEvent) => void; children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createMemo<T>(compute: () => T): Accessor<T>;
  export function For<T>(props: { each: readonly T[]; children: (item: T) => any }): any;
}
