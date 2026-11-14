import { createSignal } from "solid-js";
import { Router, useLocation } from "dep-router";

function Read() {
  const location = useLocation();
  const key = ["sea", "rch"].join("");
  const holder: { slot: Record<string, unknown>; change(): unknown } = {
    slot: {},
    change() {
      return super.slot;
    },
  };
  void location;
  holder.change();
  Object.defineProperty(holder.slot, key, { value: "inert" });
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}
export function App() { return <Router><Read /></Router>; }
