// ADR 0511 review (false positive, Chrome-confirmed): the read goes through a module-scope useVals() result settled before
// the mount; the body's own useVals() call creates another memo. rc.17 warns STRICT_READ_UNTRACKED, no pending throw.
import { createRenderEffect, createRoot } from "solid-js";
import { useVals } from "./helpers";

const shared = useVals();
createRoot(() => {
  createRenderEffect(() => shared.a1(), (v) => {
    void v;
  });
});
await new Promise((done) => setTimeout(done, 50));

export function C1() {
  const mine = useVals();
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    shared.a1();
  });
  return (
    <div>
    </div>
  );
}
