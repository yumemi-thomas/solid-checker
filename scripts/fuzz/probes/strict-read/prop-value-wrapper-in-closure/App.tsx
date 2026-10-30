// Fuzzer source axis (false clean): a getter prop (`r1={r1()}`) read by a wrapper arrow that a closure calls in the body; Solid reports STRICT_READ_UNTRACKED at the read (the read in the body itself, or through the wrapper alone, is reported; a closure returning `props.r1` is reported too).
import { createSignal } from "solid-js";

type C1Props = { r1: number };

function C1(props: C1Props) {
  const wrapUse = () => { props.r1; };
  const f1 = () => {
    wrapUse();
  };
  f1();
  return (
    <div>
    </div>
  );
}

export function P1() {
  const [r1, setR1] = createSignal(3);
  return (
    <div>
      <C1 r1={r1()} />
    </div>
  );
}
