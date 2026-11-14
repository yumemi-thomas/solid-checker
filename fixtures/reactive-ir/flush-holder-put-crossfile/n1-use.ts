import { runStep } from "steplib";
import { putN1 } from "./n1-put";

// N1: a named import, a direct call; the holder is read by its own name in a step the package runs.
const n1Holder: any = {};
putN1(n1Holder);
void runStep(() => {
  n1Holder.n1();
});
