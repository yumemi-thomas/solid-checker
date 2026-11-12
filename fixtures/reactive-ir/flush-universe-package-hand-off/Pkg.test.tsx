import { flush } from "solid-js";
import { describe, expect, it } from "vitest";

// The runner's registration is a reviewed fresh-stack caller (`TestRunner`): a flush directly in the callback is silent. `expect` is not a registration: a callback handed to
// it is handed to a package the build does not read (vitest's `expect` runs it, and may run it under a step), so its flush is uncertifiable (the lead of the proof batch:
// 147 of the 154 rows B3 adds to the corpus are `expect(() => { …; flush() })` in compass-ui's `store.test.ts`).
describe("flush", () => {
  it("in the callback", () => {
    flush();
  });

  it("in a callback handed to expect", () => {
    expect(() => {
      flush();
    }).not.toThrow();
  });
});
