declare namespace JSX {
  interface IntrinsicElements {
    div: Record<string, unknown>;
    button: Record<string, unknown>;
  }
  interface Element {}
}

// The value overload and first return slot preserve the published rc.9
// Refreshable<Accessor<T>> brand. Other overloads/options and the unused setter
// are reduced; no proof in this fixture depends on their signatures.
declare module "solid-js" {
  const $REFRESH: unique symbol;
  type Refreshable<T> = T & { readonly [$REFRESH]: any };
  type Accessor<T> = () => T;
  type SourceAccessor<T> = Refreshable<Accessor<T>>;
  type Setter<T> = (value: T) => void;
  export function createOptimistic<T>(
    value: Exclude<T, Function>
  ): [get: SourceAccessor<T>, set: Setter<T>];
}
