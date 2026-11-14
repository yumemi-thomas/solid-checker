import { createEffect, createMemo } from "solid-js";
import * as helpers from "./helpers";

const pause = () => Promise.resolve();

// Batch 11, slice 9B (ADR 0973): the typed reason that ends a site written after an async function's own suspension with no row (the site never runs), and the calls that keep
// theirs. The twin `after-await-universe` holds the sites that keep their row. This program holds a tracked async computation (`Tracked`); there is no census reason (the
// twin `after-await-universe-census`) and no callee reason.
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

// No callee reason: a project function whose whole closure (every call exactly resolves to a project function again) has no read of the universe, no generator, no JSX,
// no tagged template, no construct and no primitive still keeps the row of its call. The first build ended these calls and the review of #233 found that unsound: a read reaches
// a continuation through values no call shows (the programs of `after-await-universe`'s `ByClosure`).
function double(n: number) {
  return n * 2;
}
function quad(n: number) {
  return double(double(n));
}
// A method of a `const` object literal nothing writes a member of is exact (ADR 0279): the call is a site, and keeps its row.
const calc = {
  halve(n: number) {
    return n / 2;
  },
};
export async function Pure() {
  await pause();
  double(2);
  quad(2);
  helpers.triple(2);
  calc.halve(4);
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
