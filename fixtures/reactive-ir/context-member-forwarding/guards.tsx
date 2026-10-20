import { createContext, createSignal, useContext } from "solid-js";
const Nullish = createContext<{ nullishRead: () => number }>();
export function NullishProvider() {
  const [nullishRead] = createSignal(1);
  return <Nullish value={{ nullishRead }}><p>provider</p></Nullish>;
}
function useNullish() {
  const value = useContext(Nullish);
  if (value === undefined) throw new Error("missing");
  return value;
}
export function NullishBody() {
  const value = useNullish();
  const n = value.nullishRead();
  return <p>{n}</p>;
}
const Coalesce = createContext<{ coalesceRead: () => number }>();
export function CoalesceProvider() {
  const [coalesceRead] = createSignal(1);
  return <Coalesce value={{ coalesceRead }}><p>provider</p></Coalesce>;
}
function useCoalesce() {
  const value = useContext(Coalesce);
  value ?? (() => { throw new Error("missing"); })();
  return value;
}
export function CoalesceBody() {
  const value = useCoalesce();
  const n = value.coalesceRead();
  return <p>{n}</p>;
}
