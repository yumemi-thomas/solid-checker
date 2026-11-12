import { flush } from "solid-js";
import { describe, it } from "vitest";

// Batch 11, slice 5: a callback a reviewed test runner schedules runs from the runner's own stack (ADR 0438): a flush written directly in it has no step, no
// apply and no leaf callback below it (`TestRunner`), and so has one in the suite body the runner runs inside the registration (the census gives it the same
// class). A helper only the tests call is the census's `TestHelper`: a call to it may stand in an action's step inside a test, so no reason covers it and its
// flush is an uncertifiable row of each of the three codes.
describe("flush", () => {
  flush();

  it("in the callback", () => {
    flush();
  });

  function drain() {
    flush();
  }

  it("in a helper", () => {
    drain();
  });
});
