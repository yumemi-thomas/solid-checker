import { createMemo, createOptimistic } from "solid-js";

// Batch 11, slice 4: a site is ended only by a finding that answers its own Chrome code. Each function below holds a site that ended before for want of a row of its own
// rule: another table's row (or another rule's finding) stood at its span.

// A read ended before only by the async-read table's row: that row answers SC5003's code, not STRICT_READ_UNTRACKED. The `try` around the pending read swallows its throw
// wherever the carrier stands (`caught-handed-read`, ADR 0838), so SC5003 is uncertifiable and SC5008 stands; SC1001's own row, `strict-read-unaccounted`, stands beside them.
export function CaughtRead() {
  const pending = createMemo(async () => 1);
  return (
    <p>
      {(() => {
        const handed = () => {
          try {
            pending();
          } catch {}
        };
        [1].forEach(handed);
        return 1;
      })()}
    </p>
  );
}

// A write ended before only by another rule's finding: the async optimistic setter's call before the first answer is SC2009's violation (a rule that claims
// OPTIMISTIC_WRITE_BEFORE_LOAD), and the write's own code, REACTIVE_WRITE_IN_OWNED_SCOPE, has no row: SC2001's `write-unaccounted` stands beside the violation.
export function OptimisticWrite() {
  const [, setValue] = createOptimistic(async () => 1);
  setValue(2);
  return <p>x</p>;
}
