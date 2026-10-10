// `invokesCallerAccessor`'s *described* `callbacks` closure — one item, the
// invocation of parameter 0 at the call event on the same stack (ADR 0100) —
// and the attempt to contradict it.
//
// The closure names exactly one invocation and where it happens, so the
// runtime has two ways to contradict it: the export runs a caller-supplied
// callable the enumeration does not name (nothing else is supplied here), or
// it runs the described callable *outside* the call — after the export has
// returned, on a later turn. The recipe hands the export a callable that knows
// whether the call is on the stack, and emits `callback-invocation` only for
// a run outside it. The described invocation itself is the census's to prove
// against the one `parameter-rooted` site in the export's body; nothing here
// can add to that, and a clean run is exactly the non-contradiction the veto
// is for.
import { invokesCallerAccessor } from "implementation-census-reads-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  let inCall = false;
  let runs = 0;
  const read = () => {
    runs += 1;
    if (!inCall) {
      harness.emit({ marker: "callback-invocation", kind: "call", phase: "enter" });
    }
    return 7;
  };
  inCall = true;
  const result = invokesCallerAccessor(read);
  inCall = false;
  if (result !== 7 || runs !== 1) {
    throw new Error("invokesCallerAccessor sample disagrees");
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
