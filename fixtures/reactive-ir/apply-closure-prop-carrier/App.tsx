import { createEffect, createSignal } from "solid-js";
import type { Accessor } from "solid-js";

// ADR 1222/1229: an empty literal runs the callback zero times: the read is unentered, uncertifiable (rc.21 reads nothing).
function CEmpty(props: { r1: Accessor<number> }) {
  createEffect(
    () => 1,
    () => {
      ([] as number[]).forEach(() => {
        props.r1();
      });
    },
  );
  return <div />;
}

// ADR 1222/1229: a one-element literal runs it once in place, in the apply: an untracked read (rc.21 warns
// STRICT_READ_UNTRACKED). A violation.
function CSeeded(props: { r1: Accessor<number> }) {
  createEffect(
    () => 1,
    () => {
      [1].forEach(() => {
        props.r1();
      });
    },
  );
  return <div />;
}

export function P1() {
  const [r1] = createSignal(3);
  return (
    <div>
      <CEmpty r1={r1} />
      <CSeeded r1={r1} />
    </div>
  );
}
