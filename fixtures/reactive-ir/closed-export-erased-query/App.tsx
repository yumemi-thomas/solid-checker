import { createSignal } from "solid-js";
import { describe, observed, escaped } from "./helpers";
export type Result = ReturnType<typeof describe>;
export type Signature = typeof describe;
export const runtimeKind = typeof observed;
export const kept = [escaped];

export function App() {
  const [count] = createSignal(0);
  const value = describe({ label() { return String(count()); } });
  observed({ label() { return "fixed"; } });
  escaped({ label() { return "fixed"; } });
  return <div>{value}</div>;
}
