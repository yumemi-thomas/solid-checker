import { runStep } from "steplib";

// N6: the function is read from a dynamic import's namespace.
const { putN6 } = await import("./n6-put");
const n6Holder: any = {};
putN6(n6Holder);
void runStep(() => {
  n6Holder.n6();
});
