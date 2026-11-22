import { createSignal } from "solid-js";

type C1Props = { r1: number };

const [r1, setR1] = createSignal(3);
class Foo { then() {} } Foo.prototype.then = () => {};;

function C1(props: C1Props) {
  const f1 = () => {
    (props as typeof props).r1;
  };
  setTimeout(f1, 0);
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
