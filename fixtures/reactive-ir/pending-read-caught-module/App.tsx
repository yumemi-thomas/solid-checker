import { createMemo } from "solid-js";
import { render } from "@solidjs/web";

// `render` runs `Caught` under its root with no strict-read window: the
// pending read throws a plain NotReadyError, which the catch swallows.
const memo = createMemo(async () => 1);
const Caught = () => {
  try {
    memo();
  } catch {}
  return <i />;
};
render(Caught, document.createElement("div"));

// The same module: a catch in it may be on the stack of this read too.
const Inline = () => {
  memo();
  return <i />;
};
render(Inline, document.createElement("div"));

// A component body emits PENDING_ASYNC_UNTRACKED_READ before it throws.
export function Body() {
  const local = createMemo(async () => 1);
  try {
    local();
  } catch {}
  return <p>{1}</p>;
}
