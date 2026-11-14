import { createMemo } from "solid-js";
import { clock, makeClock, makeNested } from "fz-get";
import { clock as twinClock } from "fz-get-twin";
import { clock as computedClock } from "fz-get-computed";
import { clock as proxyClock } from "fz-get-proxy";
import { ticker } from "fz-get-define";
import { Counter } from "fz-get-class";
import { opened } from "fz-get-open";
import { calm, makeCalm } from "fz-get-clear";

// A module-scope read has no owner.
export const first = clock.now;

// S2: a getter a package declares runs its code where the project reads the member.
export function GetterBody() {
  void clock.now; // the closure defines `get now()`
  return null;
}
export function GetterInMemo() {
  createMemo(() => clock.now);
  return null;
}
export function AccessorForms() {
  void computedClock.anything; // a computed key: every name
  void proxyClock.anything; // a Proxy: every name
  void ticker.tick; // Object.defineProperty(ticker, "tick", { get })
  const counter = new Counter();
  void counter.value; // a class getter, on a value the package made
  return null;
}
export function NotClosed() {
  void opened.b; // the closure reads `process`, a global the libraries do not declare: any member may be an accessor
  return null;
}

// S2: an object pattern reads each key it names off the value, so a getter runs there as at a member read; a rest element and a computed key read any. The call
// that made the value is at module scope, where nothing runs under an owner: the pattern is where the getter runs.
const made = makeClock();
const madeNested = makeNested();
const calmMade = makeCalm();
export function Destructured() {
  const { now } = made; // the closure defines `get now()`
  return null;
}
export function DestructuredRest() {
  const { other, ...rest } = made; // a rest element reads every own enumerable key
  return null;
}
export function DestructuredNested() {
  const { other: renamed, now: { toFixed } } = made; // a nested pattern reads its keys off the member's value
  return null;
}
export function DestructuredImport() {
  const { now } = clock; // an import that may be a function is a hand-off site already: one row, not two
  return null;
}

// S2 (the third batch of reviews of #231): a pattern reads each key it names where V8 puts the frame, whatever form it takes, and the row stands at the pattern (the lane maps
// a frame on a binding to the pattern's start): renamed, with a default, assigned to names that exist, nested with a default, written as a parameter. A value the program
// imports is also the hand-off site it always was (an import may be a function): a second row, at the import's use, never instead of the pattern's.
export function PatternRenamed() {
  const { now: n } = made;
  void n;
  return null;
}
export function PatternDefault() {
  const { now = 0 } = made;
  void now;
  return null;
}
export function PatternAssigned() {
  let n = 0;
  ({ now: n } = made);
  void n;
  return null;
}
export function PatternAssignedShort() {
  let now = 0;
  ({ now } = made);
  void now;
  return null;
}
export function PatternNestedDefault() {
  const {
    inner: { now = 0 },
  } = madeNested;
  void now;
  return null;
}
export function PatternParameter() {
  (({ now = 0 }: { now: number }) => now)(made);
  return null;
}
export function ImportDefault() {
  const { now = 0 } = clock;
  void now;
  return null;
}
export function ImportAssigned() {
  let n = 0;
  ({ now: n } = clock);
  void n;
  return null;
}

// Silent: the accessors the closure defines are under other names, or the closure is clear, or the read is a legal action site.
export function PatternDefaultOther() {
  const { other = 0 } = made; // a data property, whatever the pattern's form
  void other;
  return null;
}
export function PatternAssignedOther() {
  let o = 0;
  ({ other: o } = made);
  void o;
  return null;
}
export function PatternParameterOther() {
  (({ other }: { other: number }) => other)(made);
  return null;
}
export function DestructuredOther() {
  const { other } = made; // `other` is a data property
  return null;
}
export function DestructuredClear() {
  const { now } = calmMade; // the closure names no Solid package
  return null;
}
export function Twin() {
  void twinClock.now;
  return null;
}
export function DefinedOther() {
  void ticker.plain;
  return null;
}
export function ClassData() {
  const counter = new Counter();
  void counter.plain;
  return null;
}
export function ClearPackage() {
  void calm.now;
  return null;
}
export function Handler() {
  return <button onClick={() => void clock.now}>go</button>;
}
