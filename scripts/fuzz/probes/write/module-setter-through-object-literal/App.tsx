// Fuzzer source axis (false clean): a module-scope setter imported from helpers.ts, called through an object-literal destructuring in the component body; Solid reports REACTIVE_WRITE_IN_OWNED_SCOPE (the same shape with a component-local signal, and the import called directly, are reported).
import { setW1, w1 } from "./helpers";

export function C1() {
  const { useIt } = { useIt: setW1 };
  useIt(7);
  return <div>{w1()}</div>;
}
