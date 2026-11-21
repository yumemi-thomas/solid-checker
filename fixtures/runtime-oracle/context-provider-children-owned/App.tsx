import { createContext, createMemo, onCleanup } from "solid-js";
import { ApplyPkg, LeafPkg } from "r1147-leaf";

const Ctx = createContext<number>();

function Kid() {
  createMemo(() => 1);
  onCleanup(() => {});
  return <i />;
}

// The provider element stored and handed to a package that reads its children in a tracked effect: Kid's creation and cleanup run in the children memo.
export function StoredLeaf() {
  const made = (
    <Ctx value={1}>
      <Kid />
    </Ctx>
  );
  return <LeafPkg>{made}</LeafPkg>;
}

// One component down (k9's form), read in a tracked effect.
function Inner() {
  return (
    <Ctx value={1}>
      <Kid />
    </Ctx>
  );
}
export function InnerLeaf() {
  const made = <Inner />;
  return <LeafPkg>{made}</LeafPkg>;
}

// Read in an effect's apply (SC6002's guard: inEffectCallback with no owner).
export function StoredApply() {
  const made = (
    <Ctx value={1}>
      <Kid />
    </Ctx>
  );
  return <ApplyPkg>{made}</ApplyPkg>;
}
