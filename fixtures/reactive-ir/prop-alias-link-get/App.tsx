import { createStore } from "solid-js";

// A store handed by prop and held in a `const` alias: a Get of the alias's link (`held.a`) reads the store's proxy in the component
// body, where rc.21 warns STRICT_READ_UNTRACKED. Main left the Get inside a deeper write (`held.a.v = 9`) with no row of any rule: the
// value-use census took the write's SC2003 row for the whole chain. A write's row answers its Set only; the Get keeps its own row.

type Items = { a: { v: number }; items: number[] };

// The Get alone: an SC9012 obligation (value-held-in-binding) on main already.
function ReadChild(props: { s1: Items }) {
  const held = props.s1;
  void held.a;
  return <i />;
}

// The Get inside a deeper write: the same obligation now (main: no row).
function WriteChild(props: { s1: Items }) {
  const held = props.s1;
  held.a.v = 9;
  return <i />;
}

// A write of the alias's own property: a Set with no Get of a link, the write's row alone.
function RootWriteChild(props: { s1: Items }) {
  const held = props.s1;
  held.items = [1];
  return <i />;
}

export function Parent() {
  const [s1] = createStore({ a: { v: 0 }, items: [0] });
  return (
    <div>
      <ReadChild s1={s1} />
      <WriteChild s1={s1} />
      <RootWriteChild s1={s1} />
    </div>
  );
}
