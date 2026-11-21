import { createMemo, resolve } from "solid-js";

// ADR 1159: a creation in an element's inline event handler literal, or in an arrow it calls where it is written, runs on the event's delivery: the dev
// build is quiet when the buttons are clicked, and the checker is silent there (`effect-creation-runs-off-apply`). No apply here may dispatch an event.
export function Handler() {
  return (
    <button
      onClick={() => {
        createMemo(() => 1);
      }}
    >
      x
    </button>
  );
}
export function HandlerResolve() {
  return (
    <button
      onClick={() => {
        resolve(() => 2);
      }}
    >
      r
    </button>
  );
}
export function HandlerIife() {
  return (
    <button
      onClick={() => {
        (() => {
          createMemo(() => 3);
        })();
      }}
    >
      i
    </button>
  );
}
