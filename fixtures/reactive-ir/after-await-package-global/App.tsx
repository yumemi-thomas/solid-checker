import { createSignal } from "solid-js";
import { mock } from "fz-mock";

const [flag] = createSignal(1);
const pause = () => Promise.resolve();
mock(flag);

// ADR 0999: `library-runs-nothing` (ADR 0976) ends a call of a standard-library declaration written after an async function's suspension when it runs no project
// code. The declaration is the library's, but what runs is whatever holds the global at run time: `fz-mock` replaces `structuredClone` and calls the accessor the
// program handed it, so the call reads `flag` after the await (Chrome rc.21: UNTRACKED_READ_AFTER_AWAIT, the project frame is the call).

// U (read-after-await-unaccounted): the global is one an installed package the program loads writes.
export async function Replaced() {
  await pause();
  structuredClone({ n: 1 });
}

// no row: globals no package writes keep the reason.
export async function Untouched() {
  await pause();
  JSON.stringify({ n: 1 });
  Math.max(1, 2);
}
