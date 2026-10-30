// Fuzzer source axis (false clean): a module-scope accessor imported from helpers.ts, aliased and called in the component body; Solid reports STRICT_READ_UNTRACKED (the import called directly is reported, and so is the alias of a component-local signal).
import { r1 } from "./helpers";

export function C1() {
  const useIt = r1;
  const v = useIt();
  return <div>{v}</div>;
}
