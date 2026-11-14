import { runStep } from "steplib";
import { relayTwin2 } from "./twin2-mid";

// Twin, silent: through a third file, the step never reads the member.
const twin2Holder: any = {};
relayTwin2(twin2Holder);
void runStep(() => {
  void twin2Holder.other;
});
