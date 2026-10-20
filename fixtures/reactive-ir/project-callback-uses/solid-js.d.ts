declare namespace JSX {
  interface IntrinsicElements { div: {}; button: { onClick?: () => unknown } }
}

declare module "solid-js" {
  // Each callback slot keeps the shape of solid-js@2.0.0-rc.13's published
  // `@solidjs/signals` declarations, where
  // `ComputeFunction<Prev, Next> = (v: Prev) => Next` and
  // `EffectFunction<Prev, Next> = (v: Next, p?: Prev) => (() => void) | void`.
  // Options parameters are omitted. The fixture also type-checks against the
  // published declarations (README).
  export function createSignal<T>(value: Exclude<T, Function>): [() => T, (value: T | ((prev: T) => T)) => T];
  export function createSignal<T>(fn: (v: T | undefined) => T): [() => T, (value: T | ((prev: T) => T)) => T];
  export function createMemo<T>(compute: (v: T | undefined) => T): () => T;
  export function createEffect<T>(
    compute: (v: T | undefined) => T,
    effectFn: (v: T, p?: T) => (() => void) | void,
  ): void;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
  // The first overload of the published `mapArray`, with `Accessor<T>` and
  // `Maybe<T>` written out.
  export function mapArray<Item, MappedItem>(
    list: () => readonly Item[] | undefined | null,
    map: (value: Item, index: () => number) => MappedItem,
    options?: { keyed?: true; fallback?: () => any; name?: string },
  ): () => MappedItem[];
}
