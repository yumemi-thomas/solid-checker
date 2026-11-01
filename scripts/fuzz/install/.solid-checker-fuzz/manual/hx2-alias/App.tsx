import { createSignal } from "solid-js";
import useVals from "./helpers";

export function C1() {
  const { r1, setR1 } = useVals();
  const useIt = r1;
  return (
    <div>
      <p onClick={((() => {
        useIt();
        return 1;
      })(), () => {})} data-view="v1">x</p>
    </div>
  );
}
