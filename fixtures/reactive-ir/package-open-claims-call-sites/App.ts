import { createSignal } from "solid-js";
import { runClosed, runFirst, runSecond } from "reactive-package";

// The one importer the receipt binds. `Other.ts` reaches the same exports
// through this re-export, so its calls are calls of the accepted contract.
export { runFirst, runSecond };

const [count] = createSignal(0);

export function first() {
  // `callbacks` is open for both exports, so each callable argument is an
  // open-claims obligation. Two here, one in `Other.ts`: one finding.
  runFirst(() => {
    count();
  });
  runFirst(() => {
    count();
  });
}

export function second() {
  // One here, two in `Other.ts`: one finding, separate from `runFirst`'s.
  runSecond(() => {
    count();
  });
}

export function closed() {
  // The control: `callbacks` closed and empty, so nothing is owed here.
  runClosed(() => {
    count();
  });
}
