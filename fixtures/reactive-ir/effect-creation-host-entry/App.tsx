import { createMemo, untrack } from "solid-js";
import { render } from "@solidjs/web";
import { fromHandler } from "./helper";

// ADR 1213: a creation reached only from a host entry runs where no effect's apply is running: rc.21 sets the effect-callback flag for an apply's
// synchronous run alone. The entries: an element's inline event handler literal (no apply here may dispatch an event, ADR 1159) and a callback a reviewed
// scheduler runs on a fresh stack; between the entry and the creation, only layers that run in place on its stack (an IIFE, a local `const` whose one run
// is a direct call there, `untrack(fn)`). Each is `effect-creation-runs-off-apply`.
function HandlerNamed() {
  return (
    <button
      data-act="b1"
      onClick={() => {
        const make = () => createMemo(() => 1);
        make();
      }}
    >
      n
    </button>
  );
}
function HandlerUntrack() {
  return (
    <button
      data-act="b2"
      onClick={() => {
        untrack(() => {
          createMemo(() => 2);
        });
      }}
    >
      u
    </button>
  );
}
function HandlerTimer() {
  return (
    <button
      data-act="b3"
      onClick={() => {
        setTimeout(() => {
          document.body.dataset.timer = "ran";
          createMemo(() => 3);
        }, 0);
      }}
    >
      t
    </button>
  );
}
function HandlerMicro() {
  return (
    <button
      data-act="b4"
      onClick={() => {
        queueMicrotask(() => {
          document.body.dataset.micro = "ran";
          createMemo(() => 4);
        });
      }}
    >
      m
    </button>
  );
}

// Not this proof: an exported helper (an importer the program does not show may call it from an apply) keeps its row.
function HandlerExported() {
  return (
    <button data-act="b5" onClick={() => fromHandler()}>
      e
    </button>
  );
}

render(
  () => (
    <p>
      <HandlerNamed />
      <HandlerUntrack />
      <HandlerTimer />
      <HandlerMicro />
      <HandlerExported />
    </p>
  ),
  document.body,
);
