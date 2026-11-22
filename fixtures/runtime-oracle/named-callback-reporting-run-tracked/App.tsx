import { createEffect, resolve, until } from "solid-js";

// ADR 1166 r2 (census false cleans `barm-carried-body`): `resolve`/`until` in a function an effect's compute runs tracked and a
// carried `forEach` runs untracked. The compute run throws (RESOLVE/UNTIL_IN_REACTIVE_SCOPE); the untracked run proves nothing
// about it, so SC2004/SC2005 keep their uncertifiable row (`function-run-unplaced`), never silent.
export function TwinResolveComputeCarried() {
  const f1 = () => {
    resolve(() => 1);
  };
  createEffect(f1, () => {});
  [f1].forEach((g) => g());
  return <div />;
}

export function TwinUntilComputeCarried() {
  const f1 = () => {
    until(() => 1);
  };
  createEffect(f1, () => {});
  [f1].forEach((g) => g());
  return <div />;
}
