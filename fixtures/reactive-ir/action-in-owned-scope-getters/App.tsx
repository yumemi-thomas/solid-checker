import { createMemo } from "solid-js";
import { clock } from "fz-get";
import { clock as twinClock } from "fz-get-twin";
import { clock as computedClock } from "fz-get-computed";
import { clock as proxyClock } from "fz-get-proxy";
import { ticker } from "fz-get-define";
import { Counter } from "fz-get-class";
import { opened } from "fz-get-open";
import { calm } from "fz-get-clear";

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

// Silent: the accessors the closure defines are under other names, or the closure is clear, or the read is a legal action site.
export function OtherName() {
  void clock.other;
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
