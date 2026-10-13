import { createSignal, onCleanup } from "solid-js";

declare const flag: boolean;
const noop = () => {};

// The `createRAF` shape (`@solid-primitives/raf` 3.x), without its
// `isServer` guard: one unconditional return of a complete literal whose
// slot 1 reads the signal as its first statement.
export function makeLoop() {
  const [running, setRunning] = createSignal(false, { ownedWrite: true });
  const start = () => {
    if (running()) return;
    setRunning(true);
  };
  const stop = () => {
    setRunning(false);
  };
  onCleanup(stop);
  return [running, start, stop] as const;
}

export { makeLoop as default };

const [count] = createSignal(0);

// An inline arrow slot.
export function makeInline() {
  return [count, () => count() + 1] as const;
}

// A slot that reads, for the tracked-scope negatives.
export function makePeek() {
  const peek = () => count();
  return [count, peek] as const;
}

// A slot that reads nothing.
export function makeQuiet() {
  const ping = () => {};
  return [count, ping] as const;
}

// Two return sites behind an undecidable guard (ADR 0282 slice 2 is a
// host-constant fold; nothing decides `flag`).
export function makeGuarded() {
  const [running, setRunning] = createSignal(false, { ownedWrite: true });
  const start = () => {
    if (running()) return;
    setRunning(true);
  };
  if (flag) return [running, noop, noop] as const;
  return [running, start, noop] as const;
}

// A hole in the returned literal.
export function makeHoled() {
  const peek = () => count();
  return [count, , peek] as const;
}

// A spread in the returned literal moves every later position.
export function makeSpread() {
  const peek = () => count();
  const head = [count] as const;
  return [...head, peek] as const;
}

// The literal is stored, then returned.
export function makeStored() {
  const peek = () => count();
  const result = [count, peek] as const;
  return result;
}

// A `function` expression slot: its `this` is outside the summary.
export function makeExpression() {
  const peek = function () {
    return count();
  };
  return [count, peek] as const;
}

// A module binding, not one the factory declares.
const shared = () => count();
export function makeShared() {
  return [count, shared] as const;
}

// A `let` slot binding can be replaced before the return.
export function makeReplaceable() {
  let peek = () => count();
  if (flag) peek = () => 0;
  return [count, peek] as const;
}

// A factory that forwards another factory's tuple.
export function makeForward() {
  return makePeek();
}

// A module-level destructuring another file imports: the call site and the
// pattern are in different files.
export const [, modulePeek] = makePeek();

// An accessor after a spread: the spread holds two plain functions, so the
// accessor is at runtime index 2, not 1.
export function makeSpreadAccessor() {
  const head = [() => 0, () => 0] as const;
  return [...head, count] as const;
}
