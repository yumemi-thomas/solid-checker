// app-game grease-pencil-typegpu LayerSection.tsx:36, MaterialSection.tsx:38: a handler hands an updater callback
// to a function its parent passes down, and the callback calls a helper with its document argument. The
// helper's use of the argument is placed only while no dispatch site of the project withholds the
// handler (ADR 0356), here dispatch-sites.ts's.
type Item = { id: string; name: string };
type Doc = { items: Item[]; active: string };
type Updater = (change: (doc: Doc) => Doc) => void;

function setActive(doc: Doc, id: string): Doc {
  return doc.items.some((item) => item.id === id) ? { ...doc, active: id } : doc;
}

function replaceActive(doc: Doc, change: (item: Item) => Item): Doc {
  return { ...doc, items: doc.items.map((item) => (item.id === doc.active ? change(item) : item)) };
}

function rename(doc: Doc, name: string): Doc {
  return replaceActive(doc, (item) => ({ ...item, name }));
}

function Sidebar(props: { updateDocument: Updater; name: string }) {
  return (
    <ul>
      <li onClick={() => props.updateDocument((doc) => setActive(doc, "a"))}>a</li>
      <li onClick={() => props.updateDocument((doc) => rename(doc, props.name))}>b</li>
    </ul>
  );
}

export function Panel() {
  return <Sidebar name="b" updateDocument={(change) => console.debug(change({ items: [], active: "a" }))} />;
}
