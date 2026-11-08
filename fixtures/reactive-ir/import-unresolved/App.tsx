import { createEffect } from "solid-js";
// @ts-ignore: the module is absent (a generated or virtual module)
import { helper, Helper, store } from "./missing";
// @ts-ignore
import missingDefault from "./missing-default";
// @ts-ignore
import * as Missing from "./missing-namespace";
// @ts-ignore
import { onlyType } from "./missing-type";
import { box, Local, local } from "./local";

// Positive: every runtime use of an unresolved import is open
// (SC9012 import-unresolved), whatever the position.
export function Unresolved() {
  const read = helper();
  createEffect(helper, () => {});
  createEffect(() => missingDefault(), () => {});
  store.v = 2;
  Missing.go();
  return (
    <button onClick={() => { store.v = 3; }}>
      {read}
      {helper()}
      <Helper value={1} />
    </button>
  );
}

// Negative: the same uses of a resolved import, and an unresolved one used
// only in a type position (erased), are not this row.
export function Resolved() {
  const read = local();
  createEffect(local, () => {});
  box.v = 2;
  const typed: typeof onlyType = 0;
  void typed;
  return (
    <button onClick={() => { box.v = 3; }}>
      {read}
      {local()}
      <Local value={1} />
    </button>
  );
}
