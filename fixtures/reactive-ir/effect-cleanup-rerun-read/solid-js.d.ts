// Declarations reduced from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17
// to the overloads this fixture calls: `createSignal` to its value overload
// (`dist/types/signals.d.ts:302`), `createEffect` (`:427`) and
// `createRenderEffect` (`:457`) to a compute and an effect function,
// `createTrackedEffect` (`:503`) and `untrack` (`dist/types/core/core.d.ts:167`).

declare namespace JSX {
  interface IntrinsicElements {
    button: { children?: any; onClick?: (event: MouseEvent) => void };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createEffect<const T>(compute: (prev: T | undefined) => T, effect: (value: T, prev?: T) => void | (() => void)): void;
  export function createRenderEffect<const T>(compute: (prev: T | undefined) => T, effect: (value: T, prev?: T) => void | (() => void)): void;
  export function createTrackedEffect(compute: () => void | (() => void)): void;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
}
