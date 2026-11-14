import { runStep } from "steplib";
import { putN3 } from "./n3-barrel";

// N3: the function is re-exported by name from another module.
const n3Holder: any = {};
putN3(n3Holder);
void runStep(() => {
  n3Holder.n3();
});
