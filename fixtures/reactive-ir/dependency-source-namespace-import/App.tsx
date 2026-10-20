import { createSignal } from "solid-js";
import * as Widget from "dep-ns/widget";

// Every runtime reference is a static member: each binds to the selected
// source. A type-position name (`Widget.RootProps`) is erased.
export function StaticMembers() {
  const [count] = createSignal(0);
  const now = Widget.readNow(() => count());
  Widget.readLater(() => count());
  return <Widget.Root>{now}</Widget.Root>;
}

export function Frame(props: Widget.RootProps) {
  return <Widget.Root>{props.children}</Widget.Root>;
}
