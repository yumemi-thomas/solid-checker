declare namespace JSX {
  interface IntrinsicElements { div: {} }
  interface Element {}
}

declare module "solid-js" {
  export function createMemo<T>(fn: () => T): () => T;
  export function createSignal<T>(value: T): [() => T, (next: T) => void];
  // Deliberately looser than `@solidjs/signals@2.0.0-rc.0`, which brands the
  // target: `refresh<T>(target: Refreshable<T>)` and
  // `affects(target: Accessor<unknown> | Store<object>)` /
  // `affects<T extends object>(target: Store<T>, key: keyof T)`. The invalid
  // targets these fixtures write are TS2345 against the real signatures, which
  // is why the SC7003/SC7004/SC9003 family was removed. Nothing here proves a
  // finding from the looseness any more: what remains is the `refresh(...)`
  // *write*, whose targets are all valid. Do not add a rule that depends on it.
  export function refresh(target: unknown): void;
  export function affects(target: unknown, key?: PropertyKey): void;
}
