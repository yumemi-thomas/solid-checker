import { createSignal } from "solid-js";
import { Router, useLocation } from "dep-router";

// The check of #229 (B3): a method runs with the holder as `this` where it is called, wherever it is written. `this.slot = location` in `ch` makes `holder.slot` the package's
// `location`, so the descriptor write through it (a key the program does not name) can replace `search`: the member is not stable, and each read is an obligation, never a
// violation. The holder's methods are the census of the literal's own text only when they are written in it.

// a method written outside the literal, named by a property
function Outside() {
  const location = useLocation();
  const key = ["sea", "rch"].join("");
  function ch(this: { slot: unknown }) {
    this.slot = location;
  }
  const holder: { slot: Record<string, unknown>; change(): void } = {
    slot: {},
    change: ch,
  };
  holder.change();
  Object.defineProperty(holder.slot, key, { value: "inert" });
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}

// the same by a shorthand
function OutsideShorthand() {
  const location = useLocation();
  const key = ["sea", "rch"].join("");
  function change(this: { slot: unknown }) {
    this.slot = location;
  }
  const holder: { slot: Record<string, unknown>; change(): void } = {
    slot: {},
    change,
  };
  holder.change();
  Object.defineProperty(holder.slot, key, { value: "inert" });
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}

// a method replaced after the literal
function Replaced() {
  const location = useLocation();
  const key = ["sea", "rch"].join("");
  function ch(this: { slot: unknown }) {
    this.slot = location;
  }
  const holder: { slot: Record<string, unknown>; change(): void } = {
    slot: {},
    change() {},
  };
  holder.change = ch;
  holder.change();
  Object.defineProperty(holder.slot, key, { value: "inert" });
  const [query] = createSignal(location.search);
  return <p>{query()}</p>;
}

export function App() {
  return (
    <Router>
      <Outside />
      <OutsideShorthand />
      <Replaced />
    </Router>
  );
}
