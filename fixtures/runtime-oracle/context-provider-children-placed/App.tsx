import { createContext, flush, Show } from "solid-js";
import { LeafPkg } from "r1123-leaf";

// ADR 1123, the review of round 2 (N1), through an ordinary package the build cannot analyse (an SC9005 import) that reads `props.children` in a
// tracked effect (`LeafPkg`: `children(() => props.children)` in `createTrackedEffect`). rc.21's provider returns the children memo, so the flush
// in a provider's child expression runs where that memo is READ: in the package's tracked effect, whatever component or control the provider
// element sits in. Chrome reports FLUSH_IN_TRACKED_QUEUE_CALLBACK at each flush (`Inner`'s, shared by the first three, and `ShowApp`'s own). Run alone, the
// withdrawn provider-children arm ended the SC3001 and SC2006 rows at the flush of each of the four programs and SC2008 at the first two; every row stays
// now. This case's verdict is covered-uncertifiable on that build too (the oracle's own entry renders every export and adds rows at the render), so
// the pin that fails it is the checker-side one, fixture reactive-ir/context-provider-children-refusals.

const Ctx = createContext<number>();

// The provider is the whole return of a component only tags render; its element is stored, then handed to the package.
function Inner() {
  return <Ctx value={1}>{(flush(), 1)}</Ctx>;
}
export function StoredApp() {
  const made = <Inner />;
  return <LeafPkg>{made}</LeafPkg>;
}

// The same element in an array handed on.
export function ArrayApp() {
  const list = [<Inner />];
  return <LeafPkg>{list}</LeafPkg>;
}

// The provider two components down, the outer element stored and handed on.
function Mid() {
  return <Inner />;
}
export function NestedApp() {
  const made = <Mid />;
  return <LeafPkg>{made}</LeafPkg>;
}

// The provider the child of a dialect control, the control's element stored and handed on.
export function ShowApp() {
  const made = (
    <Show when={true}>
      <Ctx value={1}>{(flush(), 1)}</Ctx>
    </Show>
  );
  return <LeafPkg>{made}</LeafPkg>;
}
