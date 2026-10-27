import { createMemo, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

// A call in the parameter list of a module-level function runs in that
// function's activation when a caller omits the argument. Called so from a
// memo, the write throws (REACTIVE_WRITE_IN_OWNED_SCOPE); called so from a
// component body, the read warns (STRICT_READ_UNTRACKED).
function listWrite(x: unknown = setCount(1)) {
  return x;
}
export function ModuleWrite() {
  const m = createMemo(() => {
    listWrite();
    return 1;
  });
  return <p>{m()}</p>;
}
function bodyRead() {
  return count();
}
function viaList(x: number = bodyRead()) {
  return x;
}
export function ModuleRead() {
  const v = viaList();
  return <p>{v}</p>;
}
