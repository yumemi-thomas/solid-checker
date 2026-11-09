import { onSettled } from "solid-js";

// ADR 0926 (ADR 0902's callee row): `onSettled.call(this, cb)` after an
// `await` runs with no owner, as the direct call does; SC4001 names it.
export function SettledCallAfterAwait() {
  const later = async () => {
    await Promise.resolve();
    onSettled.call(null, () => () => {});
  };
  later();
  return <p />;
}

// Negative: the same call in the component body, before any `await`, runs
// under the component's owner.
export function SettledCallInBody() {
  onSettled.call(null, () => () => {});
  return <p />;
}
