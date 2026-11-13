import { createSignal } from "solid-js";
import { Router, useLocation } from "dep-router";

// Twin: `change` writes nothing of `slot`, so the descriptor write targets the fresh object and `search` stays the package's getter: a violation.
function NoWrite() {
  const location = useLocation();
  const key = ["sea", "rch"].join("");
  const holder: { slot: Record<string, unknown>; change(): void } = {
    slot: {},
    change() {
      void location;
    },
  };
  holder.change();
  Object.defineProperty(holder.slot, key, { value: "inert" });
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}

// Twin: a read through `super`, and a write of another key, replace nothing the target is projected from: violations.
function SuperRead() {
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
function SuperOtherKey() {
  const location = useLocation();
  const key = ["sea", "rch"].join("");
  const holder: { slot: Record<string, unknown>; other: unknown; change(): void } = {
    slot: {},
    other: undefined,
    change() {
      (super.other as unknown) = location;
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
      <NoWrite />
      <SuperRead />
      <SuperOtherKey />
    </Router>
  );
}
