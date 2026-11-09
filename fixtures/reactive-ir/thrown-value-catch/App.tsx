import { createMemo, createSignal } from "solid-js";
import * as Ns from "./Ns";
import { readArray, readCreated, readObject, trackedArray, trackedCreated } from "./Readers";

export function DefaultReads() {
  readArray();
  readObject();
  readCreated();
  return <p>x</p>;
}

export function DefaultTracked() {
  const a = trackedArray();
  const c = trackedCreated();
  return <p>{a()}{c()}</p>;
}

// Positive: a namespace member thrown and called in the body.
export function NamespaceThrown() {
  try { throw Ns.nsCount; } catch (e) { void (e as () => number)(); }
  return <p>x</p>;
}

// Negative: the same in a memo's compute.
export function NamespaceTracked() {
  const m = createMemo(() => { try { throw Ns.nsCount; } catch (e) { return (e as () => number)(); } });
  return <p>{m()}</p>;
}

// Shadowed: the call names an inner `e`, not the catch's: no row there.
export function Shadowed() {
  const [c] = createSignal(1);
  try { throw c; } catch (e) { void e; { const e = () => 1; void e(); } }
  return <p>x</p>;
}

// Wrapper: the `throw` is in a function the `try` calls, so it leaves that
// function and no caller's `catch` is walked: the row stays at the `throw`.
export function ThrownByCallee() {
  const [c] = createSignal(1);
  const raise = () => { throw c; };
  try { raise(); } catch (e) { void (e as () => number)(); }
  return <p>x</p>;
}

// Positive: thrown out of an anonymous function called on the spot, so it
// goes on from that call into this `catch`.
export function ThrownFromIife() {
  const [c] = createSignal(1);
  try { void (() => { throw c; })(); } catch (e) { void (e as () => number)(); }
  return <p>x</p>;
}

// A `catch` pattern, and a `catch` with no binding: the row stays at the
// `throw`; the second drops the value.
export function CaughtPattern() {
  const [c] = createSignal(1);
  try { throw { c }; } catch ({ c: d }) { void (d as () => number)(); }
  try { throw c; } catch { void 0; }
  return <p>x</p>;
}
