import { createContext, createSignal, useContext } from "solid-js";
import type { JSX } from "@solidjs/web";

function make(): { rows: () => number } {
  const [rows] = createSignal(1);
  return { rows };
}
// A key the literal fact cannot name could be any member: each exact mutation
// builtin below, even uninvoked, vetoes every transported member.
function templateKey(target: { state: () => number }) {
  Object.defineProperty(target, `state`, { value: () => 0 });
}
function identifierKey(target: object, key: string) { Reflect.set(target, key, () => 0); }
export function FactoryConsumer() {
  const store = make();
  const value = store.rows();
  return <p>{value}</p>;
}
const Context = createContext<{ state: () => number }>();
export function Provider(props: { children?: JSX.Element }) {
  const [state] = createSignal(1);
  return <Context value={{ state }}>{props.children}</Context>;
}
export function ContextConsumer() {
  const account = useContext(Context);
  const value = account.state();
  return <p>{value}</p>;
}
// A direct signal read is independent of transported member stability.
export function DirectBody() {
  const [count] = createSignal(1);
  const value = count();
  return <p>{value}</p>;
}
