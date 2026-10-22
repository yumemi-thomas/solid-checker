import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Panel() {
  const value = count();
  return <b>{value}</b>;
}

function TabsRoot() {
  const value = count();
  return <div>{value}</div>;
}

function TabsList() {
  const value = count();
  return <ul>{value}</ul>;
}

// Uncertifiable: the module's `Parts` is rendered nowhere. The tag below names
// the parameter that shadows it, so it proves nothing about `Panel`.
const Parts = { Panel };

export function Screen(Parts: { Panel: () => unknown }) {
  return <Parts.Panel />;
}

// Uncertifiable: this `Object` is not the standard library's, so `Tabs` is
// not an `Object.assign` result.
const Object = {
  assign<T extends () => unknown, U>(target: T, _members: U): T & U {
    return target as T & U;
  },
};

const Tabs = Object.assign(TabsRoot, { List: TabsList });

export function Shell() {
  return (
    <Tabs>
      <Tabs.List />
    </Tabs>
  );
}
