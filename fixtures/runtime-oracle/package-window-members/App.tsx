import { createMemo, createSignal } from "solid-js";
import "fz-wview";
import "fz-wsource";
import "fz-wviewclear";
import "fz-wdocview";

// ADR 1108: a global the library declares, which an installed package replaces through a member the DOM types as this realm's window: an event's `view`, a message's
// `source`. The scan takes that member for the global object, as it takes `defaultView` (ADR 0974).
export function WrittenThroughEventView() {
  void btoa("x"); // `event.view.btoa = …`
  return null;
}
// The message is delivered after the module loads: the memo calls `atob` when it recomputes, after the write.
export function WrittenThroughMessageSource() {
  const [ready, setReady] = createSignal(false);
  setTimeout(() => setReady(true), 100);
  const decoded = createMemo(() => (ready() ? atob("eA==") : "")); // `message.source.atob = …`
  return <p>{decoded()}</p>;
}

// Unchanged (ADR 0974): `defaultView` read off any object.
export function WrittenThroughDefaultView() {
  void unescape("x");
  return null;
}

// Silent: the writer is clear (its closure names no Solid package), the global is one no package writes, or the call runs where no owner is active.
export function WrittenByClearPackage() {
  void escape("x"); // `fz-wviewclear` writes `escape` through `event.view` and cannot call an action
  return null;
}
export function Unwritten() {
  void encodeURIComponent("x");
  return null;
}
export function WrittenInHandler() {
  return <button onClick={() => btoa("x")}>encode</button>;
}
