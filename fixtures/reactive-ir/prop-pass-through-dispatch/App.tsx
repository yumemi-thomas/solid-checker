import { evens } from "./helpers";

// Clean: every tag that renders `List` passes an array built at the tag, or
// a parent's prop that holds one.
function List(props: { items: number[] }) {
  const picked = evens(props.items);
  return <div>{picked.length}</div>;
}

// `Middle` forwards its own prop, and its only tag passes a literal.
function Middle(props: { items: number[] }) {
  return <List items={props.items} />;
}

// `SC9012`: a spread may supply `items`.
function Spread(props: { items: number[] }) {
  const picked = evens(props.items);
  return <div>{picked.length}</div>;
}

// `SC9012`: one tag passes the root's prop, which no tag shows.
function Mixed(props: { items: number[] }) {
  const picked = evens(props.items);
  return <div>{picked.length}</div>;
}

// `SC9012`: handed out as a value, so something else may render it.
function Routed(props: { items: number[] }) {
  const picked = evens(props.items);
  return <div>{picked.length}</div>;
}
export const routes = [Routed];

export function App(props: { items: number[] }) {
  const shared = { items: [4] };
  return (
    <div>
      <List items={[1, 2]} />
      <Middle items={[3]} />
      <Spread {...shared} />
      <Mixed items={[5]} />
      <Mixed items={props.items} />
      <Routed items={[6]} />
    </div>
  );
}
