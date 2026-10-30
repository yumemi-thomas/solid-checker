import { createContext, createSignal, useContext } from "solid-js";
import type { Accessor, Setter } from "solid-js";
import { useCounter, useOptimistic } from "./hook";

// Positive: a hook's setter and action, destructured or as members, written
// and called in the component body (SC2001, the action violation).
export function Destructured() {
  const { setCount, save } = useCounter();
  setCount(2);
  save();
  return <p />;
}

export function Member() {
  const counter = useCounter();
  counter.setCount(3);
  counter.save();
  return <p />;
}

// Positive: a context value's setter, with one provider (a closed program).
type Vals = { count: Accessor<number>; setCount: Setter<number> };
const CountContext = createContext<Vals>();

function Reader() {
  const vals = useContext(CountContext)!;
  vals.setCount(4);
  return <p />;
}

export function Provider() {
  const [count, setCount] = createSignal(0);
  return (
    <CountContext value={{ count, setCount }}>
      <Reader />
    </CountContext>
  );
}

// Negative: the same calls in a handler run outside the owned scope.
export function InHandler() {
  const { setCount, save } = useCounter();
  return <button onClick={() => { setCount(5); save(); }} />;
}

// Negative: an optimistic setter meets no owned-scope guard on rc.17.
export function Optimistic() {
  const { setCount } = useOptimistic();
  setCount(6);
  return <p />;
}
