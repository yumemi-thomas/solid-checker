import { createSignal, createStore } from "solid-js";

declare const dynamicKey: "data" | "then";

// Review round 1 of ADR 0293: the `then` exemption is the trap's own
// warning. The trap then evaluates the backing property, and a getter there
// runs its own reads, so an exempt Get is withheld only where the store's
// initial literal proves the property is data that runs nothing.
const [count] = createSignal(0);

// Uncertifiable (SC9012): the backing `then` may run code.
export function GetterThen() {
  const [q] = createStore({
    get then() {
      return count();
    },
  });
  const value = q.then;
  return <p>{value}</p>;
}
const backing = {
  get then() {
    return count();
  },
};
export function UnprovenThenBacking() {
  const [q] = createStore(backing);
  const value = q.then;
  return <p>{value}</p>;
}
export function CalledReadingThen() {
  const [q] = createStore({ then: () => count(), data: 1 });
  q.then();
  return <p>ok</p>;
}
// The setter is used, so the backing may be rewritten after it is built.
export function WrittenStoreThen() {
  const [q, setQ] = createStore({ then: 1, data: 1 });
  const value = q.then;
  return <button onClick={() => setQ((s) => void (s.data = 2))}>{value}</button>;
}

// Silent: the backing `then` is data, or absent, and nothing rewrites it.
export function ThenData() {
  const [q] = createStore({ then: 1, data: 1 });
  const value = q.then;
  return <p>{value}</p>;
}
export function AbsentThen() {
  const [q] = createStore({ data: 1 });
  const value = (q as { then?: number }).then;
  return <p>{value}</p>;
}

// Review round 1 of ADR 0293: an undecided key in a helper's summary is an
// uncertifiable read at the call, never silence and never a violation.
export function HelperUndecidedKey() {
  const [q] = createStore({ data: 0 });
  const key = "data";
  function read() {
    return q[key];
  }
  const value = read();
  return <p>{value}</p>;
}
export function ClosureUndecidedKey() {
  const [q] = createStore({ then: () => {}, data: 1 });
  const read = () => q[dynamicKey];
  const value = read();
  return <p>{String(value)}</p>;
}
// A warning Get beside an undecided one in the same helper stays proven.
export function HelperMixedKeys() {
  const [q] = createStore({ data: 0 });
  const key = "data";
  function read() {
    return q.data + q[key];
  }
  const value = read();
  return <p>{value}</p>;
}
