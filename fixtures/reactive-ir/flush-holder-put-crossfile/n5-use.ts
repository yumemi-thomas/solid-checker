import { runStep } from "steplib";
import putN5 from "./n5-put";

// N5: a default export, imported under a name.
const n5Holder: any = {};
putN5(n5Holder);
void runStep(() => {
  n5Holder.n5();
});
