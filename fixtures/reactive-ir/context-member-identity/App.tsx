import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";
import { TabsContext, useTabs } from "./tabs";

export function TabsProvider(props: { children?: JSX.Element }) {
  const [active] = createSignal("a");
  const [orientation] = createSignal("h");
  return <TabsContext value={{ active, orientation }}>{props.children}</TabsContext>;
}

// Positive: the context and its wrapper are published through an export
// list in a closed program, and the comparison helpers below read a member
// without exposing the shared provider object.
export function TabsList() {
  const ctx = useTabs();
  const orientation = ctx.orientation();
  return <p>{orientation}</p>;
}

export function TabsTrigger() {
  const ctx = useTabs();
  const isActive = () => ctx.active() === "a";
  function isLocked() {
    return ctx.active() !== "b";
  }
  return <button disabled={isActive()} onClick={() => isLocked()}>tab</button>;
}
