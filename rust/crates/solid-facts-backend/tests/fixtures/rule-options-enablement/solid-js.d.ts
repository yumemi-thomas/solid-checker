// Byte-faithful to the signatures this fixture's proof depends on in
// solid-js@2.0.0-rc.3: the plain `createSignal` overload excludes functions
// (a function initializer is the writable-memo form), and `createEffect`
// takes a compute arm and an effect arm. A looser stub would invent a defect
// no real project can produce.
declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: Exclude<T, Function>): [Accessor<T>, Setter<T>];
  export function createEffect<T>(
    compute: (previous: T | undefined) => T,
    effect: (value: T) => void
  ): void;
}
