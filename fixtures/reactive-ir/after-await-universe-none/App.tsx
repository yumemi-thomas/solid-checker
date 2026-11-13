import { createEffect, createMemo } from "solid-js";

const pause = () => Promise.resolve();

// Batch 11, slice 9B (ADR 0973): the typed reasons that end a site written after an async function's own suspension with no row. The twin `after-await-universe` holds
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

// The callee reads nothing: a project function whose whole closure (every call exactly resolves to a project function again) has no read of the universe, no
// generator, no JSX, no tagged template, no construct and no primitive.
function double(n: number) {
  return n * 2;
}
function quad(n: number) {
  return double(double(n));
}
export async function Pure() {
  await pause();
  double(2);
  quad(2);
}

// The site never runs: it follows an unconditional exit, and sits in an arm a literal test rules out. The callee is a project function that calls the standard library, so
// the callee reason does not end these two.
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
