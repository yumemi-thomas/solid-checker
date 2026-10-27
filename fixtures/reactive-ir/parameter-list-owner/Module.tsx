import { createMemo, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

// ---- A call in the parameter list of a function runs in that function's
// activation when a caller omits the argument. A module-level function has no
// function around it: its list is no module code, and a call made so from a
// memo or a component body must not be silent (ADR 0454; rc.17 throws
// REACTIVE_WRITE_IN_OWNED_SCOPE and warns STRICT_READ_UNTRACKED).

function bodyWrite() {
  setCount(1);
  return 1;
}
export function BodyWriteControl() {
  const m = createMemo(() => bodyWrite());
  return <p>{m()}</p>;
}
function listWrite(x: unknown = setCount(1)) {
  return x;
}
export function ModuleListWrite() {
  const m = createMemo(() => {
    listWrite();
    return 1;
  });
  return <p>{m()}</p>;
}
export function listWriteExported(x: unknown = setCount(1)) {
  return x;
}
export function ExportedListWrite() {
  const m = createMemo(() => {
    listWriteExported();
    return 1;
  });
  return <p>{m()}</p>;
}
export function NestedListWrite() {
  function w(x: unknown = setCount(1)) {
    return x;
  }
  const m = createMemo(() => {
    w();
    return 1;
  });
  return <p>{m()}</p>;
}

function bodyRead() {
  return count();
}
export function BodyReadControl() {
  const v = bodyRead();
  return <p>{v}</p>;
}
function viaList(x: number = bodyRead()) {
  return x;
}
export function ModuleViaListRead() {
  const v = viaList();
  return <p>{v}</p>;
}
export function NestedViaListRead() {
  function rd() {
    return count();
  }
  function vl(x: number = rd()) {
    return x;
  }
  const v = vl();
  return <p>{v}</p>;
}
// The argument is supplied: the default does not run (no violation).
export function ModuleViaListReadSupplied() {
  const v = viaList(1);
  return <p>{v}</p>;
}
// Legal, and uncertifiable all the same (a cost): the helper's default read is
// tracked here, but nothing proves the call omits the argument.
function listRead(x: number = count()) {
  return x;
}
export function ModuleListReadTracked() {
  const m = createMemo(() => listRead());
  return <p>{m()}</p>;
}
