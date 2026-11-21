import { createMemo, resolve } from "solid-js";
import { render } from "@solidjs/web";
import { fromHandler } from "./helper";

// ADR 1159: a creation in an element's inline event handler literal runs on the event's delivery, never on an effect's apply: no apply here may dispatch
// an event, and the literal has no binding, so its delivery is its one entry (`effect-creation-runs-off-apply`). An arrow the handler calls where it is
// written runs on the same stack.
function Handler() {
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
function HandlerResolve() {
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
function HandlerIife() {
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

// Named layers answer nothing here: a local helper the handler calls, and an exported helper (an importer the program does not show may call it from an
// apply) keep their rows.
function HandlerNamed() {
  return (
    <button
      onClick={() => {
        const make = () => createMemo(() => 4);
        make();
      }}
    >
      n
    </button>
  );
}
function HandlerExported() {
  return <button onClick={() => fromHandler()}>e</button>;
}

render(
  () => (
    <p>
      <Handler />
      <HandlerResolve />
      <HandlerIife />
      <HandlerNamed />
      <HandlerExported />
    </p>
  ),
  document.body,
);
