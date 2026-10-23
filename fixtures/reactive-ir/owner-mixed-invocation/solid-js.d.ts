// Declarations transcribed from solid-js@2.0.0-rc.17 and @solidjs/signals@2.0.0-rc.17.
//
// Byte-faithful, because the claims here rest on an `onCleanup` and a
// `runWithOwner` callback being well typed:
//
// - `onCleanup`, `Accessor`, `SourceAccessor`, `Setter`, `Signal`
//                  @solidjs/signals `dist/types/signals.d.ts:47`, `:55-56`,
//                  `:66-73` (doc comments dropped)
// - `Disposable`   `dist/types/core/types.d.ts:3-5`
// - `Refreshable`  `dist/types/core/constants.d.ts:219-221` (its `$REFRESH`
//                  brand is a local `unique symbol`)
// - `runWithOwner` `dist/types/core/core.d.ts:340`
// - `getOwner`, `createRoot`
//                  `dist/types/core/owner.d.ts:66`, `:110`
//
// Reduced, and safe to reduce because no claim reads them back: `Owner` keeps
// two of the published interface's members (`core/types.d.ts:153-155`);
// `createSignal` keeps only its plain-value overload without `options`
// (`dist/types/signals.d.ts:302`); `JSX` keeps the elements and the event
// attributes the cases write (`@solidjs/web` types its `on*` attributes more
// loosely than `(event: Event) => void`; none of them is looser for any
// handler written here).

declare namespace JSX {
  interface IntrinsicElements {
    button: {
      children?: unknown;
      onClick?: (event: MouseEvent) => void;
    };
    div: {
      children?: unknown;
      onClick?: (event: MouseEvent) => void;
      onScroll?: (event: Event) => void;
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
  export interface Owner {
    id?: string;
    _config: number;
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
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
  export function getOwner(): Owner | null;
  export function createRoot<T>(init: ((dispose: () => void) => T) | (() => T), options?: {
      id?: string;
      transparent?: boolean;
  }): T;
  export function createSignal<T>(value: Exclude<T, Function>): Signal<T>;
}
