import { createSignal } from "solid-js";

type C1Props = { r1: number };

function C1(props: C1Props) {
  const f1 = async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        (props as typeof props).r1;
      }
      await Promise.resolve();
    }
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
