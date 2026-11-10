// SC5008 caught-reactive-read. A reactive read is not an `await`: its throw is how a
// compute suspends to a `<Loading>` boundary, and a `try` that swallows it makes the
// compute complete with a value. rc.21 reports CAUGHT_REACTIVE_READ for it at the
// line the compute is created on (`createMemo(() => {`), not at the read inside the
// `try`, so the oracle covers the diagnostic by file (ADR 0946). The control
// rethrows, which lets the suspension go on; nothing is reported.
import { Loading, createMemo } from "solid-js";

export function Caught() {
  const source = createMemo(async () => 1);
  const safe = createMemo(() => {
    try {
      return source();
    } catch {
      return 0;
    }
  });
  return (
    <Loading fallback="loading">
      <p>{safe()}</p>
    </Loading>
  );
}

export function Rethrown() {
  const source = createMemo(async () => 1);
  const safe = createMemo(() => {
    try {
      return source();
    } catch (error) {
      throw error;
    }
  });
  return (
    <Loading fallback="loading">
      <p>{safe()}</p>
    </Loading>
  );
}
