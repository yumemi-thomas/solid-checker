import { createSignal } from "solid-js";
import { Router, useLocation } from "dep-router";

// The check of #227 (G2): `super.slot = v` writes the own slot of the receiver, as `this.slot = v` does. `holder.slot` is the package's `location` after `holder.change()`,
// so the descriptor write through it (a key the program does not name) can replace `search`: the member is not stable, and the read is an obligation, never a violation.
function SuperWrite() {
  const location = useLocation();
  const key = ["sea", "rch"].join("");
  const holder: { slot: Record<string, unknown>; change(): void } = {
    slot: {},
    change() {
      (super.slot as unknown) = location;
    },
  };
  holder.change();
  Object.defineProperty(holder.slot, key, { value: "inert" });
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}

export function App() {
  return (
    <Router>
      <SuperWrite />
    </Router>
  );
}
