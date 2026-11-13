import { onCleanup } from "solid-js";
import * as self from "./App";

// ADR 0970, R2 of the re-review of #221: an OPEN program (a publishing manifest) whose own timer enumerates the module's namespace with no name. The namespace census is a closed
// program's, so the owner emission's component stop found no entry for `Comp`, which a tag renders, and certified it; the timer calls it with no owner and Chrome rc.21 reports
// NO_OWNER_CLEANUP. `Inner` is no export: the namespace does not reach it, and it is quiet.
export function Comp() {
  onCleanup(() => {});
  return <p>comp</p>;
}
function Inner() {
  onCleanup(() => {});
  return <p>inner</p>;
}
export function Host() {
  return (
    <p>
      <Comp />
      <Inner />
    </p>
  );
}
setTimeout(() => {
  for (const component of Object.values(self)) {
    (component as () => unknown)();
  }
}, 0);
