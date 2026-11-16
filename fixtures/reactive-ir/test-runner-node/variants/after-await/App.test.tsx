import { createMemo, createSignal } from "solid-js";
import { describe, test } from "node:test";

const [count] = createSignal(0);
const pause = () => Promise.resolve();
const reads = () => count();

// ADR 1004. `node:test` records a test and runs it later from its own loop (ADR 0438, `Test.start`): the call after the await ends, `runner-scheduled`.
test("a scheduled test", async () => {
  await pause();
  reads();
});

// `node:test` runs a suite body inside the `describe` call (ADR 0438, `createBuild`): the body's stack is the registration's own, which may be a computation's
// flight, so the call after the await keeps SC1002's uncertifiable row, `read-after-await-unaccounted`.
describe("an inline suite body", async () => {
  await pause();
  reads();
});

// A registration written after an await in an async computation: the call of `test` is that computation's own site (a call into a package on its flight), and keeps
// SC1002's uncertifiable row. The callback it registers is scheduled like any other.
export const registers = () =>
  createMemo(async () => {
    await pause();
    await test("registered on a flight", async () => {
      await pause();
      reads();
    });
    return 1;
  });
