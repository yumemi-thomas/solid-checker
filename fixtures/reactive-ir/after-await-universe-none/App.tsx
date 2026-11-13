import { createEffect, createMemo } from "solid-js";

const pause = () => Promise.resolve();

// Batch 11, slice 9B (ADR 0973): the typed reason that ends a site written after an async function's own suspension with no row. The twin `after-await-universe` holds
// the sites that keep their row. This program holds a tracked async computation (`Tracked`); there is no census reason (the twin `after-await-universe-census`).
export function Tracked() {
  const m = createMemo(async () => {
    await pause();
    return 1;
  });
  createEffect(
    () => m(),
    () => {},
  );
  return <div />;
}

// The site never runs: it follows an unconditional exit, and sits in an arm a literal test rules out.
function stringify(n: number) {
  return JSON.stringify({ n });
}
export async function Dead() {
  await pause();
  if (false) {
    stringify(1);
  }
  return 1;
  stringify(2);
}
