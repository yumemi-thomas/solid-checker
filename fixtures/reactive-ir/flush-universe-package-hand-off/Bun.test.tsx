import { flush } from "solid-js";
import { describe, it } from "bun:test";

// `bun:test` is native: no published JavaScript shows when it runs a suite body (`SuiteBodyRun::Unproven`), so `test_runner_callback` answers nothing for `describe`. The
// registration is a reviewed runner's all the same, by its declaration (`bun-types/test.d.ts`, a registration's name): what is handed to it is the runner's own, so `drain`,
// written in the suite body, is handed to nothing a package runs: silent on the census reasons.
describe("suite", () => {
  const drain = () => {
    flush();
  };

  it("calls a helper of the suite", () => {
    drain();
  });
});
