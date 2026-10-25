// Reduced from solid-js@2.0.0-rc.9 (see rc9-until-scope): the two primitives
// whose calls in a tracked compute are SC2004 (`resolve`) and SC2005 (`until`),
// and the signal and memo they are called around.
declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export type Truthy<T> = Exclude<T, false | 0 | 0n | "" | null | undefined>;
  export function resolve<T>(fn: () => T): Promise<T>;
  export function until<T>(fn: () => T): Promise<Truthy<T>>;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
  export function createMemo<T>(compute: (prev: T | undefined) => T): Accessor<T>;
}
