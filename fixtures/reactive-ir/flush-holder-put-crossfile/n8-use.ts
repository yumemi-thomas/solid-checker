import { runStep } from "steplib";
import { relayN8 } from "./n8-mid";

// N8: the call is in a third file, whose own function is called from a fourth.
const n8Holder: any = {};
relayN8(n8Holder);
void runStep(() => {
  n8Holder.n8();
});
