import { flush } from "solid-js";
import { it } from "vitest";
import { useClock } from "./clock";
import { schedule } from "./Scheduler";

// Not withheld, whatever the clock is: module evaluation runs before any callback (`ModuleInitialization`), a test runner's callback is run by the runner (`TestRunner`), and
// the code past an `await` its own async function reaches resumes from the promise job queue, which no fake clock replaces (`AwaitContinuation`).
flush();

it("runs on the runner's stack", () => {
  useClock();
  schedule();
  flush();
});

export async function later() {
  await Promise.resolve();
  flush();
}
