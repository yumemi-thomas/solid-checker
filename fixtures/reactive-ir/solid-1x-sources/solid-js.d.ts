declare namespace JSX {
  interface IntrinsicElements { div: { children?: unknown } }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T) => void];
  export function createMemo<T>(fn: () => T): () => T;
  type EffectFunction<Prev, Next extends Prev = Prev> = (v: Prev) => Next;
  export function createEffect<Next, Init = Next>(fn: EffectFunction<Init | Next, Next>, value?: Init): void;
  export function createResource<T>(fetcher: () => Promise<T>): [() => T | undefined, unknown];
  export function createDeferred<T>(fn: () => T): () => T;
  export function createComputed<T>(fn: (v?: T) => T | Promise<T>, value?: T): void;
  export function For<T>(props: { each: T[]; children: (item: T) => JSX.Element }): JSX.Element;
  export function Index<T>(props: { each: T[]; children: (item: () => T) => JSX.Element }): JSX.Element;
  export function createSelector<T>(source: () => T): (key: T) => boolean;
}
declare module "solid-js/store" {
  export function createMutable<T extends object>(value: T): T;
}
