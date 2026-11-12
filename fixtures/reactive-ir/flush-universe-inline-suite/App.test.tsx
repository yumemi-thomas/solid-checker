import { flush } from "solid-js";
import { describe } from "node:test";

// Batch 11, slice 5 (the second re-review of #213, N4): `node:test` runs a suite body inside the `describe` call, at module evaluation, so a flush written directly in
// it runs before any callback (`ModuleInitialization`, the class of the registration's position). The class says nothing of code written in the body that runs elsewhere
// or later: a class field's initializer (an instance's construction runs it), a default value (the function's call runs it), a prop written in JSX (a getter that runs where
// the component reads it) and a named suite body its own code uses (`again()` enters it again, from any stack). Each is an uncertifiable row of each code, never silence.
describe("suite", () => {
  // Silent, on the class: the flush runs when the suite body runs.
  flush();

  describe("nested", () => {
    flush();
  });

  class Field {
    x = flush();
  }
  void Field;

  const withDefault = (x = flush()) => x;
  void withDefault;

  function Child(props: { v: void }) {
    return <p>{String(props.v)}</p>;
  }
  void (<Child v={flush()} />);

  // A generator handed to an inline carrier is a function of its own, with no class (it was uncertifiable before).
  void [1].map(function* () {
    flush();
  });
});

let kept: undefined | ((second?: boolean) => void);

// A named suite body whose name its own code uses: `kept` runs it again on any stack.
describe("named", function again(second?: boolean) {
  kept = again;
  flush();
  void second;
});

// Silent: a named suite body whose name nothing uses runs once.
describe("once", function once() {
  flush();
});

export { kept };
