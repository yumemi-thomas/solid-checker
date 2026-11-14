import { runStep } from "steplib";
import { putN4 } from "./n4-barrel";

// N4: the function is re-exported by `export *`.
const n4Holder: any = {};
putN4(n4Holder);
void runStep(() => {
  n4Holder.n4();
});
