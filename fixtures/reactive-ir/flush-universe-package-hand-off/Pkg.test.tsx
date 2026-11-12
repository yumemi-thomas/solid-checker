import { flush } from "solid-js";
import { describe, expect, it } from "vitest";

// The runner's registrations (`describe`, `it`) are a reviewed runner's: a function handed to one is the runner's own, whatever the project proves of when the runner runs a
// suite body (vitest's bytes do not show it here, so `test_runner_callback` answers nothing for `describe`, and the registration is known by its declaration all the same), so
// nothing hands `drain` or the test callbacks to a package: silent on the census reasons. `expect` is no registration: a callback handed to it is handed to a package the build
// does not read (vitest's `expect` runs it), so its flush is uncertifiable (three rows).
describe("flush", () => {
  const drain = () => {
    flush();
  };

  it("in the callback", () => {
    flush();
  });

  it("in a helper of the suite", () => {
    drain();
  });

  it("in a callback handed to expect", () => {
    expect(() => {
      flush();
    }).not.toThrow();
  });
});
