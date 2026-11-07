import { createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);

declare const element: HTMLElement;

// No code of this program puts a function at `addEventListener`, so the
// host's own registration keeps its timing: the listener may run on its
// invoker's stack, an obligation, as in host-callback-read-timing before that
// fixture's own `addEventListener` method (ADR 0840).
export function ElementListener() {
  element.addEventListener("click", () => console.log(count()));
  return <div />;
}

export function ListenerAsync() {
  const user = createMemo(async () => 1);
  element.addEventListener("click", () => console.log(user()));
  return <div />;
}
