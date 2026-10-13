import { createContext, createSignal, useContext } from "solid-js";
type Value = { orientation: () => number };
const Context = createContext<Value>();
export function Provider() {
  const [orientation] = createSignal(1);
  return <Context value={{ orientation }}><p>provider</p></Context>;
}
export function useValue(): Value {
  const value = useContext(Context);
  if (!value) throw new Error("missing context");
  return value;
}
