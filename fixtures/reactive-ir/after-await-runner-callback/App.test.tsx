import { createMemo, createRoot } from "solid-js";
import { beforeEach, describe, expect, it, it as check, test } from "vitest";
import { spec } from "./runner";
import { count, pause, reads, setCount, state } from "./signals";

// ADR 1004: a site written after an await directly in a function literal a reviewed test runner SCHEDULES (ADR 0438: vitest calls a registered test or hook, and a suite
// body, from its own loop; a registered `async` callback is wholly in the premise, its prefix and every resumption) is on no async computation's flight: the runtime raises
// UNTRACKED_READ_AFTER_AWAIT only for a read with a flight's frame on its async stack, and that stack is the runner's. Such a site ends (`runner-scheduled`): no row.

// Ended: a store read, a call of a project function, a call into a package, a construct and a setter, each directly in the callback. (The accessor read `count()` keeps
// the row another producer files for it, `function-run-unplaced`: the reason ends only what no row stands at.)
it("a read", async () => {
  await pause();
  count();
  void state.n;
  reads();
  expect(count()).toBe(0);
  void new Map([[1, reads]]);
  setCount(1);
});

// Ended: a hook, a suite body vitest defers to its collection, a modifier, a curried registration, an alias of the import and a re-export of the registration.
beforeEach(async () => {
  await pause();
  reads();
});
describe("a suite", async () => {
  await pause();
  reads();
});
test.only("a modifier", async () => {
  await pause();
  reads();
});
it.each([1, 2])("curried %i", async () => {
  await pause();
  reads();
});
check("an alias", async () => {
  await pause();
  reads();
});
spec("a re-export", async () => {
  await pause();
  reads();
});

// Kept: the callback is no literal written in the registration (a wrapper's result, a binding another reference may hand to a computation).
const wrap = (fn: () => Promise<void>) => fn;
it(
  "a wrapper",
  wrap(async () => {
    await pause();
    reads();
  }),
);
const shared = async () => {
  await pause();
  reads();
};
it("a binding", shared);

// Kept: a project function named like the runner's is not the runner (exact resolution, never a spelling).
function local(name: string, fn: () => Promise<void>) {
  void name;
  return createMemo(fn);
}
{
  const it = local;
  it("shadowed", async () => {
    await pause();
    reads();
  });
}

// Kept: a named function expression whose own code uses its name may hand itself to a computation, which then runs it on a flight.
it("named", async function again(): Promise<void> {
  await pause();
  reads();
  createRoot(() => createMemo(again));
});

// Kept: a function written inside the callback is its own function. An async computation's read after its own await is the walk's violation; a promise handler, a nested
// async arrow and a generator keep their rows.
it("nested", async () => {
  await pause();
  createRoot(() =>
    createMemo(async () => {
      await pause();
      return count();
    }),
  );
  void pause().then(() => count());
  const later = async () => {
    await pause();
    reads();
  };
  void later;
  void [1].map(function* () {
    count();
  });
});

// Kept: an async helper the tests call is a function like any other (a computation may run it too): its own sites keep their rows. The call of it in the test ends.
async function helper() {
  await pause();
  reads();
}
it("a helper", async () => {
  await pause();
  await helper();
});
export const memo = () => createMemo(helper);
