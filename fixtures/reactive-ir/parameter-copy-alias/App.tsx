import { createEffect, createSignal } from "solid-js";
import type { Accessor } from "solid-js";

type Props = { r1: Accessor<number> };

// ADR 1233: a `const` copy of a destructured prop nothing writes holds that prop: the read in an effect's apply is an untracked read
// (rc.21 STRICT_READ_UNTRACKED).
function Copied({ r1 }: Props) {
  const useIt = r1;
  createEffect(
    () => 1,
    () => {
      useIt();
    },
  );
  return <p>x</p>;
}

// Uncertifiable: the prop is written before the copy is read, so the copy holds what the call bound, which no fact names.
function Written({ r1 }: Props) {
  const useIt = r1;
  r1 = () => 2;
  createEffect(
    () => 1,
    () => {
      useIt();
      r1();
    },
  );
  return <p>x</p>;
}

// Clean: the same copy read in a click handler, where no window is open.
function InHandler({ r1 }: Props) {
  const useIt = r1;
  return <button onClick={() => useIt()} />;
}

export function Parent() {
  const [r1] = createSignal(3);
  return (
    <div>
      <Copied r1={r1} />
      <Written r1={r1} />
      <InHandler r1={r1} />
    </div>
  );
}
