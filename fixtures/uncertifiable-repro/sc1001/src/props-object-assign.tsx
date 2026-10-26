// skyjtx-website Tabs.tsx:68-70,204: a component exported only through `Object.assign(Root, {...})` (a compound component).
import { createSignal } from "solid-js";

function TabsRoot(props: { defaultValue?: string }) {
  const [value, setValue] = createSignal(props.defaultValue ?? "");
  return <div onClick={() => setValue("")}>{value()}</div>;
}

export const Tabs = Object.assign(TabsRoot, { displayName: "Tabs" });
