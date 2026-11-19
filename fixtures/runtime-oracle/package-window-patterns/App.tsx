import { createMemo, createSignal } from "solid-js";
import "fz-rview";
import "fz-rsource";
import "fz-rdocview";
import "fz-rmember";
import "fz-rread";
import "fz-rclear";


// ADR 1108 part 2: a global the library declares, replaced through a window member a pattern reads.
export function WrittenThroughViewPattern() {
  void btoa("x");
  return null;
}

// The message is delivered after the module loads: the memo calls `atob` when it recomputes, after the write.
export function WrittenThroughSourcePattern() {
  const [ready, setReady] = createSignal(false);
  setTimeout(() => setReady(true), 100);
  const decoded = createMemo(() => (ready() ? atob("eA==") : ""));
  return <p>{decoded()}</p>;
}
export function WrittenThroughDefaultViewPattern() {
  void unescape("x");
  return null;
}

// Unchanged (ADR 1108 part 1): `event.view.encodeURI = …`.
export function WrittenThroughViewMember() {
  void encodeURI("x");
  return null;
}

// Silent: a global only a clear package writes, one no package writes, and the call in a handler.
export function WrittenByClearPackage() {
  void decodeURI("x");
  return null;
}
export function Unwritten() {
  void encodeURIComponent("x");
  return null;
}
export function CalledInHandler() {
  return <button onClick={() => btoa("x")}>encode</button>;
}
