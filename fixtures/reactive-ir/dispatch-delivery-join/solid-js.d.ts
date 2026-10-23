// Declarations transcribed from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17.
//
// Byte-faithful, because the claims here rest on a handler's `onCleanup` and a
// signal write being well typed:
//
// - `onCleanup`, `Accessor`, `SourceAccessor`, `Setter`, `Signal`
//                  @solidjs/signals `dist/types/signals.d.ts:47`, `:55-56`,
//                  `:66-73` (doc comments dropped)
// - `Disposable`   `dist/types/core/types.d.ts:3-5`
// - `Refreshable`  `dist/types/core/constants.d.ts:219-221` (its `$REFRESH`
//                  brand is a local `unique symbol`)
//
// Reduced, and safe to reduce because no claim reads them back: `createSignal`
// keeps only its plain-value overload without `options`
// (`dist/types/signals.d.ts:302`), and `JSX` keeps the elements and the event
// attributes the cases write (`@solidjs/web` types its `on*` attributes more
// loosely than `(event: Event) => void`; none of them is looser for any
// handler written here).

declare namespace JSX {
  type Ref<T> = T | ((el: T) => void) | undefined;
  interface IntrinsicElements {
    div: {
      children?: unknown;
      ref?: Ref<HTMLDivElement>;
      onScroll?: (event: Event) => void;
      onClick?: (event: MouseEvent) => void;
      onPing?: (event: Event) => void;
    };
    button: {
      children?: unknown;
      onClick?: (event: MouseEvent) => void;
    };
    p: { children?: unknown };
  }
  interface Element {}
}

declare module "solid-js" {
  const $REFRESH: unique symbol;
  export type Refreshable<T> = T & {
    readonly [$REFRESH]: any;
  };
  export interface Disposable {
    (): void;
  }
  export type Accessor<T> = () => T;
  export type SourceAccessor<T> = Refreshable<Accessor<T>>;
  export type Setter<in out T> = {
    <U extends T>(...args: undefined extends T ? [] : [value: Exclude<U, Function> | ((prev: T) => U)]): undefined extends T ? undefined : U;
    <U extends T>(value: (prev: T) => U): U;
    <U extends T>(value: Exclude<U, Function>): U;
    <U extends T>(value: Exclude<U, Function> | ((prev: T) => U)): U;
  };
  export type Signal<T> = [get: SourceAccessor<T>, set: Setter<T>];
  export function onCleanup(fn: Disposable): Disposable;
  export function createSignal<T>(value: Exclude<T, Function>): Signal<T>;
}
