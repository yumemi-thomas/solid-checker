import { runStep } from "steplib";
import { putTwin } from "./twin-put";

// Twin, silent: the callers are all known and the holder is sole-named; the package step never reads the member.
const twinHolder: any = {};
putTwin(twinHolder);
void runStep(() => {
  void twinHolder.other;
});
