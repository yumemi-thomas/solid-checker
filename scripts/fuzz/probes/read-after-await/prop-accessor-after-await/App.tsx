// Fuzzer source axis (false clean): an async memo reads an accessor it received as a prop after an await; Solid reports UNTRACKED_READ_AFTER_AWAIT at the read.
import { createEffect, createMemo, createSignal } from "solid-js";
import type { Accessor } from "solid-js";

type C1Props = { r1: Accessor<number> };

function C1(props: C1Props) {
  const m1 = createMemo(async () => {
    await Promise.resolve();
    return props.r1();
  });
  createEffect(() => m1(), () => {});
  return <div />;
}

export function P1() {
  const [r1] = createSignal(3);
  return <C1 r1={r1} />;
}
