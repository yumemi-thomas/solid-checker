// The signatures this fixture's proof depends on, byte-faithful to
// solid-js@2.0.0-rc.3: `createMemo(compute, options?)` with no positional
// value parameter, `MemoOptions.equals` as the comparator slot, and the plain
// `createSignal` overload excluding functions.
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export interface MemoOptions<T> {
    equals?: false | ((prev: T, next: T) => boolean);
  }
  export function createSignal<T>(value: Exclude<T, Function>): [Accessor<T>, Setter<T>];
  export function createMemo<T>(
    compute: (previous: T | undefined) => T,
    options?: MemoOptions<T>
  ): Accessor<T>;
}
