import { createSignal } from "solid-js";

const [count] = createSignal(0);

function TabsRoot() {
  const value = count();
  return <div>{value}</div>;
}

function TabsList() {
  const value = count();
  return <ul>{value}</ul>;
}

export const Tabs = Object.assign(TabsRoot, { List: TabsList });

// Uncertifiable: `Object.assign` mutates `TabsRoot` itself, so `TabsRoot.List`
// reaches every member without naming `Tabs`; the plain call runs `TabsList`
// inside tracked JSX.
const L = (TabsRoot as typeof Tabs).List;

export function Screen() {
  return (
    <Tabs>
      <Tabs.List />
      {L()}
    </Tabs>
  );
}
