// Fuzzer source axis (false clean): `refresh` of a memo received as a prop, in the child's body; Solid reports REACTIVE_WRITE_IN_OWNED_SCOPE at the call.
import { createMemo, refresh } from "solid-js";
import type { SourceAccessor } from "solid-js";

type C1Props = { m1: SourceAccessor<number> };

function C1(props: C1Props) {
  refresh(props.m1);
  return <div>{props.m1()}</div>;
}

export function P1() {
  const m1 = createMemo(() => 1);
  return <C1 m1={m1} />;
}
