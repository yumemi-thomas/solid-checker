import { createSignal } from "solid-js";

const [count, setCount] = createSignal(0);
const [, setFlag] = createSignal(0, { equals: false });
const [, setCustom] = createSignal(0, { equals: (a, b) => a === b });
const options = { equals: false as const };
const [, setNamed] = createSignal(0, options);
const pause = () => Promise.resolve();

// Batch 11, slice 9B follow-up (ADR 0976): a call into code outside the project written after an async function's own suspension is a site, and ends in no row only where the typed
// fact `library-runs-nothing` holds: the program leaves every built-in alone (this one writes none, defines none reflectively and evaluates no string), the callee is an exactly
// resolved standard-library declaration, its receiver is the library's own (no receiver, a proven primitive or an ECMAScript global) and every argument is inert (a literal, an exact data
// literal, or a function handed to a slot that runs later). A setter is inert on the same arguments when the signal it sets has no function-valued `equals`.
//
// Each call of `Outside` ends in no row. The process test `a_library_call_that_runs_no_project_code_is_no_site` pins them, and the mutation test of the reason files exactly them.
export async function Outside() {
  await pause();
  JSON.stringify({ n: 1 });
  Promise.resolve(1);
  setCount(1);
  setFlag(1);
  new Date();
  Math.max(1, 2);
  Object.keys({ a: 1 });
  setTimeout(() => {
    count();
  }, 0);
}

// Each call of `Kept` keeps its row: the same calls with one thing the proof does not have. A value no fact proves (a parameter: a getter, a `toString` or a function may be what it
// holds), a signal whose `equals` is a function or an options value no fact shows, a spread, a receiver that is only a library type, a standard member with no resolved-call fact (an
// argumentless call demands none). No call here evaluates a string: that would be a touched built-in (`after-await-library-patched`), which takes the proof from every call.
export async function Kept(value: unknown, text: string, list: number[]) {
  await pause();
  JSON.stringify(value);
  Promise.resolve(value);
  setCount(value as number);
  setCustom(1);
  setNamed(1);
  new Date(text);
  Math.max(...list);
  list.push(1);
  text.trim();
}
