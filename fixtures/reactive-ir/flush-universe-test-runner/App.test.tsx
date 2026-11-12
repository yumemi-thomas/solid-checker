import { action, flush } from "solid-js";
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

  // The review of #213 (B1 and B2): the class says where the function around the flush runs only when the flush is in that function's body and nothing enters the function
  // again by its name. A named test callback its own code calls again runs the second time on the stack of that call (an action's step), and a default value runs wherever its
  // function is called: each flush is an uncertifiable row of each code (six rows; the default's SC2006 is the producers' own `parameter-initializer-unplaced`).
  it("named", function again(second?: unknown) {
    flush();
    if (second !== true) {
      const run = action(function* () {
        again(true);
      });
      void run();
    }
  });

  let late: undefined | ((x?: void) => void);
  it("a default", () => {
    late = (x = flush()) => x;
  });
  void late;
});
