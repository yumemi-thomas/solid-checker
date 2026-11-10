import { createMemo } from "solid-js";

// ADR 0837: a named helper's pending read, called in an anonymous closure
// handed down forwarding helpers in a tracked insert. Chrome (rc.21) reports
// ASYNC_OUTSIDE_LOADING_BOUNDARY, unplaced. The forward runs in place (ADR
// 0931), so the read is the insert's: SC5003 uncertifiable at the call (the
// build before ADR 0837 left SC5003 with no row).
export function HandedInInsert() {
  const pending = createMemo(async () => 1);
  const wrapUse = () => { pending(); };
  const hc0 = (g: () => void) => { g(); };
  const hc1 = (g: () => void) => { hc0(g); };
  return (
    <p>
      {(() => {
        hc1(() => {
          wrapUse();
        });
        return 1;
      })()}
    </p>
  );
}
