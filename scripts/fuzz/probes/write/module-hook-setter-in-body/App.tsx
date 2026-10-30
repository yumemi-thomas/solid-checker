// Fuzzer source axis (false clean): a hook in helpers.ts creates a signal and returns its setter; the component calls it in its body; Solid reports REACTIVE_WRITE_IN_OWNED_SCOPE at the call.
import { useVals } from "./helpers";

export function C1() {
  const { w1, setW1 } = useVals();
  setW1(7);
  return <div>{w1()}</div>;
}
