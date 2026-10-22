import { createSignal } from "solid-js";

const [count] = createSignal(0);

function TabsRoot(props: { children?: unknown }) {
  const value = count();
  return <div>{value}{props.children}</div>;
}

function TabsList() {
  const value = count();
  return <ul>{value}</ul>;
}

function TabsItem() {
  const value = count();
  return <li>{value}</li>;
}

// Positive: `Object.assign` from the standard library stores the members, and
// every use of `Tabs` is a tag: `<Tabs>` renders the target `TabsRoot`,
// `<Tabs.List>` and `<Tabs.Item>` the members.
export const Tabs = Object.assign(TabsRoot, {
  List: TabsList,
  Item: TabsItem,
});

export function Screen() {
  return (
    <Tabs>
      <Tabs.List>
        <Tabs.Item></Tabs.Item>
      </Tabs.List>
    </Tabs>
  );
}
