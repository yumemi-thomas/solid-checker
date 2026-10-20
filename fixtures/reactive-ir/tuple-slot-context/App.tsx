import { createContext, createSignal, useContext } from "solid-js";
import type { JSX } from "@solidjs/web";

const [count] = createSignal(0);

function makePeek() {
  const peek = () => count();
  return [count, peek] as const;
}

function makeObject() {
  const peek = () => count();
  return { count, peek };
}

const TupleContext = createContext<readonly [() => number, () => number]>();
const ObjectContext = createContext<{ count: () => number; peek: () => number }>();

export function Providers(props: { children?: JSX.Element }) {
  return (
    <TupleContext value={makePeek()}>
      <ObjectContext value={makeObject()}>{props.children}</ObjectContext>
    </TupleContext>
  );
}

// Refused: every consumer of the context shares the provider's one array,
// and any of them can replace a slot.
export function TupleConsumer() {
  const [, peek] = useContext(TupleContext);
  peek();
  return <p />;
}

// Control: the same provider census admits a returned object's member.
export function ObjectConsumer() {
  const { peek } = useContext(ObjectContext);
  peek();
  return <p />;
}
