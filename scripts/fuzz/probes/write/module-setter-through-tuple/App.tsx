// Fuzzer source axis (false clean): a module-scope setter imported from helpers.ts, called through a tuple slot in the component body; Solid reports REACTIVE_WRITE_IN_OWNED_SCOPE.
import { setW1, w1 } from "./helpers";

export function C1() {
  const pair = [0, setW1] as const;
  const [, useIt] = pair;
  useIt(7);
  return <div>{w1()}</div>;
}
