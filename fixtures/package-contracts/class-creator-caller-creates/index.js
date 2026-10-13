// A module-private class whose constructor creates, reached from two exports
// (ADR 0158 amendment): `createThing` constructs it, and `wrapThing` reaches the
// construction only by calling `createThing`, as `@tanstack/solid-router`'s
// `createFileRoute` reaches `new Route` only through `createRoute`.
import { createEffect } from "solid-js";

class Thing {
  constructor(read) {
    createEffect(read, () => {});
    this.read = read;
  }
}

export function createThing(read) {
  return new Thing(read);
}

export function wrapThing(read) {
  return createThing(read);
}

// Control: creates nothing and constructs nothing.
export function plain(value) {
  return value;
}
