// The rc.21 declarations this fixture uses, as `@solidjs/signals` 2.0.0-rc.21 publishes them (see ../../solid-js.d.ts).
declare module "solid-js" {
  const $REFRESH: unique symbol;
  export type Refreshable<T> = T & { readonly [$REFRESH]: any };
  export type Accessor<T> = () => T;
  export type SourceAccessor<T> = Refreshable<Accessor<T>>;
  export type Store<T> = T;
  export function createMemo<T>(compute: (prev: T | undefined) => T): SourceAccessor<T>;
  export function affects(target: Accessor<unknown> | Store<object>): void;
  export function affects<T extends object>(target: Store<T>, key: keyof T): void;
}
declare module "@solidjs/signals" {
  export const $TARGET: unique symbol;
}
