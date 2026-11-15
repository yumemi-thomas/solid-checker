import { createSignal } from "solid-js";
import { test } from "bun:test";

const [count] = createSignal(0);
const pause = () => Promise.resolve();
const reads = () => count();

// ADR 1004: `runner-scheduled` ends a site after an await only for a runner whose published JavaScript shows how it calls a registered callback. `bun:test` is
// native (ADR 0438: no bytes to read), so a call written after an await in its test callback keeps SC1002's uncertifiable row, `read-after-await-unaccounted`.
test("a test of a native runner", async () => {
  await pause();
  reads();
});
