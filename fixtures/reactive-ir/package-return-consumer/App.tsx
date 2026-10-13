import { createCount, createLabel } from "reactive-package";

const count = createCount();

export function Good() {
  return <div>{count()}</div>;
}

export function Bad() {
  const value = count();
  return <div>{value}</div>;
}

// `createLabel`'s contract is identical to `createCount`'s; the difference is
// here. Its result is discarded and it is never an argument, so under
// `docs/package-contract-v2/phase21/2026-09-10-sc9005-demand-scoping-design.md`
// § 8 no consumer can reach its `returns`. Nothing scopes the obligation to
// that yet, which is what the vehicle exists to make visible.
export function Effectful() {
  createLabel();
  return <div />;
}
