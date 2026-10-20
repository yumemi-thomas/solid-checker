import { createSignal } from "solid-js";

const [count] = createSignal(0);

const TabsRoot = (props: { children?: unknown }) => {
  const value = count();
  return <div>{value}{props.children}</div>;
};

function TabsList() {
  const value = count();
  return <ul>{value}</ul>;
}

// Positive: a `const` bound to an arrow is a target that takes the members as
// plain data properties, as a function declaration does.
export const Tabs = Object.assign(TabsRoot, { List: TabsList });

export function Screen() {
  return (
    <Tabs>
      <Tabs.List />
    </Tabs>
  );
}
