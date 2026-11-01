import { createSignal } from "solid-js";

export function C1() {
  const [r1, setR1] = createSignal(3);
  let useIt = r1;
  return (
    <div>
      <p onClick={((() => {
        useIt();
        return 1;
      })(), () => {})} data-view="v1">x</p>
    </div>
  );
}
