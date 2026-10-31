// A local `createSignal` shadows the star's: not the primitive.
export * from "solid-js";
export function createSignal<T>(v: T): [() => T] {
  return [() => v];
}
