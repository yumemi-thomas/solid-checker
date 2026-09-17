// The signatures this fixture's proof depends on, faithful to
// solid-js@2.0.0-rc.3: `mapArray` returns an `Accessor`, its default (keyed)
// overload passes the item by value and the index as an accessor, and the
// plain `createSignal` overload excludes functions.
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: Exclude<T, Function>): [Accessor<T>, Setter<T>];
  export function mapArray<Item, MappedItem>(
    list: Accessor<readonly Item[]>,
    map: (value: Item, index: Accessor<number>) => MappedItem,
    options?: { keyed?: true; name?: string },
  ): Accessor<MappedItem[]>;
}
