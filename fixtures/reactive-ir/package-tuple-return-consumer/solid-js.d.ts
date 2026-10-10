declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
  }
  interface Element {}
}

// The three signatures under test. The **return shape** of each is byte-faithful
// to the audited 2.0 prerelease, because the return shape is the whole claim:
//
//   @solidjs/signals/dist/types/signals.d.ts
//     export type Signal<T> = [get: SourceAccessor<T>, set: Setter<T>];
//     export declare function createOptimistic<T>(value: Exclude<T, Function>,
//                                                 options?): Signal<T>;
//   @solidjs/signals/dist/types/store/next/optimistic.d.ts
//     createOptimisticStoreNext<T extends object>(…): [get: Store<T>,
//                                                      set: StoreSetter<T>];
//   @solidjs/signals/dist/types/store/next/projection.d.ts
//     createProjectionNext<T extends object>(…): Refreshable<Store<T>>;
//
// `createProjection` is the one that returns the store *itself*, which is why
// `Dialect::returns_reactive_tuple` deliberately leaves it out. Its `Refreshable`
// wrapper is dropped here -- no claim in this fixture reads `.refresh` -- and
// that is the only place these stubs are narrower than the package.
declare module "solid-js" {
  type SourceAccessor<T> = () => T;
  type Setter<T> = (value: T) => void;
  type Store<T> = T;
  type StoreSetter<T> = (next: Partial<T>) => void;
  export function createOptimistic<T>(
    value: Exclude<T, Function>
  ): [get: SourceAccessor<T>, set: Setter<T>];
  export function createOptimisticStore<T extends object>(
    first: T
  ): [get: Store<T>, set: StoreSetter<T>];
  export function createProjection<T extends object>(
    fn: (draft: T) => void,
    seed: T
  ): Store<T>;
}
