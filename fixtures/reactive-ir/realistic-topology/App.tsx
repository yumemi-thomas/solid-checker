// A project shaped like a project: components in their own files rendered by
// other components, helpers called from component bodies, and a module-scope
// source read across files. Every finding here is decided by a fact that only
// exists because more than one file is in the program.
import { createMemo } from "solid-js";
import { Badge, Card, Plaque } from "./components/Card";
import { readCountNow, watchCount } from "./lib/hooks";
import { count } from "./lib/store";

export function Dashboard() {
  const label = createMemo(() => `count: ${count()}`);

  // Called from a component body, so the effect's owner is proven and no owner
  // obligation remains. Its read stays inside the effect and must not be
  // charged to this call.
  watchCount();

  // This one does read at the call site, in a scope that does not track.
  const snapshot = readCountNow();

  return (
    <div>
      <Card title={label()} />
      <Badge label={label()} />
      <Plaque note="static" />
      <span>{snapshot}</span>
    </div>
  );
}
