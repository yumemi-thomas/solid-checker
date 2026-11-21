import { createStore } from "solid-js";

// A store link held whole in a `const` (`const held = s1.a`) is the store's proxy below it: a Get through it (`held.v`) reads the
// store in the component body, where rc.21 warns STRICT_READ_UNTRACKED. Main had no row of any rule there (the value-use census ended
// at the chain that read the link). Each Get now has a row: SC9012 `value-held-in-binding` (uncertifiable) where nothing proves more.

// The Get through the alias (main: no row).
function AliasGet() {
  const [s1] = createStore({ a: { v: 0 } });
  const held = s1.a;
  void held.v;
  return <i />;
}

// Its member twin: an SC1001 violation, as on main.
function MemberGet() {
  const [s1] = createStore({ a: { v: 0 } });
  void s1.a.v;
  return <i />;
}

// A link of a link held: the same.
function DeepAliasGet() {
  const [s1] = createStore({ a: { b: { v: 0 } } });
  const held = s1.a.b;
  void held.v;
  return <i />;
}

export function Page() {
  return (
    <div>
      <AliasGet />
      <MemberGet />
      <DeepAliasGet />
    </div>
  );
}
