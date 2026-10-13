import { createMemo, createSignal } from "solid-js";
import {
  Base,
  bus,
  callSecond,
  callWhen,
  disableRunner,
  makeBase,
  runAfterFail,
  runAlways,
  runEach,
  runReassigned,
  runWhen,
  runWhenUnset,
  runner,
} from "./helpers";

// Arguments that prove nothing, so these cases test the rule without
// argument facts (ADR 0280 is pinned by `argument-sensitive-reads`).
declare const flag: boolean;

// ---- Proven: the helper calls the callback on every completion.

export function PassedAlways() {
  const [count] = createSignal(1);
  const value = runAlways(count);
  return <main>{value}</main>;
}
export function ArrowAlways() {
  const [count] = createSignal(1);
  const value = runAlways(() => count());
  return <main>{value}</main>;
}

// ---- Not proven: the helper calls it on some paths only.

export function PassedWhen() {
  const [count] = createSignal(1);
  const value = runWhen(flag, count);
  return <main>{value}</main>;
}
export function PassedTernary() {
  const [count] = createSignal(1);
  const value = callWhen(count, flag);
  return <main>{value}</main>;
}
export function ArrowWhen() {
  const [count] = createSignal(1);
  const value = runWhen(flag, () => count());
  return <main>{value}</main>;
}
export function ArrowEach() {
  const [count] = createSignal(1);
  const value = runEach([], () => count());
  return <main>{value}</main>;
}
export function ArrowWhenUnset() {
  const [count] = createSignal(1);
  const value = runWhenUnset({ value: 1 }, () => count());
  return <main>{value}</main>;
}
// A pending async memo read in a callback the helper may never call: no
// SC5001 error.
export function AsyncArrowWhen() {
  const user = createMemo(async () => 1);
  const value = runWhen(flag, () => user());
  return <main>{value}</main>;
}

// ---- Not proven: the argument does not provably reach the parameter the
// helper calls, or the helper may throw first.

export function AfterFailArrow() {
  const [count] = createSignal(1);
  let value = 0;
  try {
    value = runAfterFail(flag, () => count());
  } catch {
    value = -1;
  }
  return <main>{value}</main>;
}
export function ReassignedPassed() {
  const [count] = createSignal(1);
  const value = runReassigned(count);
  return <main>{value}</main>;
}
export function ReassignedArrow() {
  const [count] = createSignal(1);
  const value = runReassigned(() => count());
  return <main>{value}</main>;
}
export function SpreadBefore() {
  const [count] = createSignal(1);
  const other = () => 0;
  const value = callSecond(...([1, other] as const), () => count());
  return <main>{value}</main>;
}
export function CallMember() {
  const [count] = createSignal(1);
  const value = callSecond.call(undefined, () => count(), () => 0);
  return <main>{value}</main>;
}
export function ReplacedRunner() {
  const [count] = createSignal(1);
  disableRunner();
  const value = runner(() => count());
  return <main>{value}</main>;
}

const localBus = {
  run(cb: () => number) {
    return cb();
  },
};
const patchedBus = {
  run(cb: () => number) {
    return cb();
  },
};
Object.assign(patchedBus, { run: (_cb: () => number) => 0 });

// Proven: an unexported object-literal method and an exact class instance.
export function LocalBus() {
  const [count] = createSignal(1);
  const value = localBus.run(() => count());
  return <main>{value}</main>;
}
// Not proven: an importer may patch an exported object, and `Object.assign`
// replaces `run` without an assignment by name (ADR 0280 review round 4).
export function ExportedBus() {
  const [count] = createSignal(1);
  const value = bus.run(() => count());
  return <main>{value}</main>;
}
export function AssignedBus() {
  const [count] = createSignal(1);
  const value = patchedBus.run(() => count());
  return <main>{value}</main>;
}
export function ExactInstance() {
  const [count] = createSignal(1);
  const base = new Base();
  const value = base.run(() => count());
  return <main>{value}</main>;
}
// Not proven: the receiver may hold an override (ADR 0279).
export function MaybeOverride() {
  const [count] = createSignal(1);
  const base = makeBase(true);
  const value = base.run(() => count());
  return <main>{value}</main>;
}
