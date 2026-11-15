import { createEffect, createMemo, createRoot, createSignal, flush } from "@solidjs/signals";
import { afterAll, beforeEach, describe, expect, it } from "vitest";
import { writeFileSync } from "node:fs";

// ADR 1004, the real-runner half: the fixture's shapes under vitest's own runner on V8, with the dev build's diagnostics collected (console.warn / console.error).
const seen: { test: string; text: string }[] = [];
let current = "module";
const warn = console.warn, error = console.error;
console.warn = (...args: unknown[]) => { seen.push({ test: current, text: String(args[0]).split("\n")[0] }); };
console.error = (...args: unknown[]) => { seen.push({ test: current, text: String(args[0]).split("\n")[0] }); };

const [count, setCount] = createSignal(0, { name: "count" });
const pause = () => new Promise<void>((done) => setTimeout(done, 5));
const reads = () => count();
const settle = async () => { for (let i = 0; i < 6; i++) { await pause(); flush(); } };

beforeEach(async () => {
  await pause();
  count();
  reads();
});

describe("a suite", async () => {
  current = "suite body";
  await pause();
  count();
  reads();

  it("a read directly in a scheduled test callback", async () => {
    current = "direct";
    await pause();
    count();
    reads();
    expect(count()).toBe(0);
    await pause();
    setCount(0);
    count();
  });

  it.each([1, 2])("curried %i", async () => {
    current = "curried";
    await pause();
    count();
  });

  // The control: the diagnostics are live in this run. The same body awaited by an async memo's compute is on that computation's flight.
  it("the twin: an async computation's flight", async () => {
    current = "flight";
    await pause();
    const body = async () => {
      await pause();
      count();
    };
    createRoot(() => {
      const m = createMemo(async () => {
        await body();
        return 1;
      });
      createEffect(() => m(), () => {});
    });
    await settle();
  });

  // The fixture's `named` case: a named callback its own code hands to a computation runs again on that computation's flight.
  it("named, handed to a computation", async function again(): Promise<void> {
    const first = current !== "named-again";
    if (first) current = "named";
    await pause();
    count();
    if (first) {
      current = "named-again";
      createRoot(() => {
        const m = createMemo(again as () => Promise<void>);
        createEffect(() => m(), () => {});
      });
      await settle();
    }
  });
});

afterAll(() => {
  console.warn = warn; console.error = error;
  writeFileSync(process.env.PROBE_OUT ?? "diagnostics.json", JSON.stringify(seen, null, 1));
});
