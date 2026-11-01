// Declarations reduced from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17
// to the overloads this fixture calls, with no claim reading them back:
// `createSignal` to its value overload (`dist/types/signals.d.ts`), `createRoot`
// to its init and options (`dist/types/core/owner.d.ts`), `createEffect` to a
// compute and an effect function (`dist/types/signals.d.ts`), and `createOwner` and
// `runWithOwner` (`dist/types/core/owner.d.ts`, `core/core.d.ts`). The standard
// library (`Array.forEach`, `Function.call`, `setTimeout`, ...) is TypeScript's
// own lib.

declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: any; onClick?: (event: MouseEvent) => void };
    button: { children?: any; onClick?: (event: MouseEvent) => void };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createRoot<T>(init: ((dispose: () => void) => T) | (() => T), options?: { id?: string }): T;
  export function createEffect<const T>(compute: (prev: T | undefined) => T, effect: (value: T, prev?: T) => void | (() => void)): void;
  export interface Owner {}
  export function createOwner(options?: { id?: string }): Owner;
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
}
