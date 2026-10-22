import { createEffect, getOwner, runWithOwner } from "solid-js";

// Each expression below evaluates to exactly the binding its leading token
// names. The proofs that withhold these findings read that binding through
// the reference index, which keeps such composite entity keys.

// An absent (null) handler installs no listener: no SC1007.
export function AbsentHandler() {
  const absent = null;
  return <button onFoo-bar={absent ?? absent}>x</button>;
}

export function AbsentConditionalHandler() {
  const absent = null;
  return <button onFoo-bar={absent === absent ? absent : absent}>x</button>;
}

// The component's own owner keeps the effect owned: no SC4001.
export function ReusedOwner() {
  const owner = getOwner();
  return <button onClick={() => runWithOwner(owner ?? owner, () => createEffect(() => 1, () => {}))}>x</button>;
}

// Positive controls: the same rules are live in this project.
export function NumberHandler() {
  const count = 1;
  return <button onFoo-bar={count}>x</button>;
}

export function NullOwner() {
  return <button onClick={() => runWithOwner(null, () => createEffect(() => 1, () => {}))}>x</button>;
}
