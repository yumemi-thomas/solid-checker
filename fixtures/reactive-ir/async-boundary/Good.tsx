import { createMemo, isPending, latest, Loading } from "solid-js";

const user = createMemo(async () => ({ name: "Ada" }));

// Loading is structurally scoped: an inner boundary remains covered by the
// outer boundary, and reads in both regions are tracked and allowed to pend.
export function GoodNestedLoading() {
  return (
    <Loading fallback={<div />}>
      <div>
        {user().name}
        <Loading fallback={<div />}>{user().name}</Loading>
      </div>
    </Loading>
  );
}

// latest() and isPending() execute their readers inline. Inside Loading that
// inherited JSX execution is tracked and boundary-covered, so both are safe.
export function GoodPendingObservation() {
  return (
    <Loading fallback={<div />}>
      <div>{isPending(() => user()) ? "pending" : latest(() => user().name)}</div>
    </Loading>
  );
}
