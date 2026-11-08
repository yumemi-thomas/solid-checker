import { createSignal } from "solid-js";

// `lazy` with no server pass: a later mount in a window, the read warns.
export default function LazyBody() {
  const [a] = createSignal(0);
  const v = a();
  return <i>{v}</i>;
}
