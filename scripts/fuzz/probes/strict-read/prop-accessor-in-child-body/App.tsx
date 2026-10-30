// Fuzzer source axis (false clean): a child calls an accessor it received as a prop in its body (untracked); Solid reports STRICT_READ_UNTRACKED at the read.
import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";

type C1Props = { r1: Accessor<number> };

function C1(props: C1Props) {
  const v = props.r1();
  return <div>{v}</div>;
}

export function P1() {
  const [r1] = createSignal(3);
  return <C1 r1={r1} />;
}
