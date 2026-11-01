import { createSignal } from "solid-js";

export function C1() {
  const [r1, setR1] = createSignal(3);
  let useIt = r1;
  [1].forEach(() => {
    useIt();
  });
  return (
    <div>
    </div>
  );
}
