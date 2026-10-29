// Fuzzer async axis (false clean): a parent creates an async memo and passes the accessor; the child reads it in its body while pending; Solid reports PENDING_ASYNC_UNTRACKED_READ at the read.
import { createMemo } from "solid-js";
import type { Accessor } from "solid-js";

type C1Props = { a1: Accessor<number> };

function C1(props: C1Props) {
  props.a1();
  return <div />;
}

export function P1() {
  const a1 = createMemo(async () => 1);
  return <C1 a1={a1} />;
}
