import { createSignal } from "solid-js";

const [count] = createSignal(0);
const [items] = createSignal(["a", "b"]);

// Positive: rendered by its tag, so its body is an untracked component body.
function Tagged() {
  const value = count();
  return <b>{value}</b>;
}

// Uncertifiable: handed to `map` inside tracked JSX, so it runs as a plain
// call in that tracked scope (runtime-probed: no STRICT_READ_UNTRACKED). The
// reference is no tag, declaration or export, so identity is undecided.
function Row(item: string) {
  const prefix = count();
  return <li>{prefix}:{item}</li>;
}

// Uncertifiable: an object method is reached through its object by any
// alias, so its name's references are no census of its entries.
const renderers = {
  badge() {
    const value = count();
    return <b>{value}</b>;
  },
};

// Uncertifiable: handed as a project component's prop, which that component
// calls as a plain function inside tracked JSX. The prop's spelling says
// nothing; only a dialect renderer (`<Dynamic component>`) renders it.
function Cell(item: string) {
  const value = count();
  return <b>{value}:{item}</b>;
}

function List(props: { component: (item: string) => unknown }) {
  return <ul>{items().map(props.component)}</ul>;
}

export function App() {
  return (
    <ul>
      <Tagged />
      {items().map(Row)}
      <p>{renderers.badge()}</p>
      <List component={Cell} />
    </ul>
  );
}
