// Clean. Mirrors donegeon-client web/apps/client/src/page/BoardControllerDeck.tsx:168-182: a chain of memos,
// each reading the one before (a rank table, a filtered list, an ordered copy), consumed in JSX.
import { createMemo, createSignal, For } from "solid-js";

function Deck() {
  const [stacks, setStacks] = createSignal([{ id: "b", def: "pack" }, { id: "a", def: "deck" }]);
  const rankByDef = createMemo(() => ({ deck: 0, pack: 1 }) as Record<string, number>);
  const deckStacks = createMemo(() => stacks().filter((stack) => stack.def.length > 0));
  const ordered = createMemo(() => {
    const rank = rankByDef();
    return [...deckStacks()].sort((a, b) => (rank[a.def] ?? 9) - (rank[b.def] ?? 9));
  });
  return (
    <ul>
      <For each={ordered()}>{(stack) => <li>{stack.id}</li>}</For>
      <button type="button" onClick={() => setStacks([...stacks(), { id: "c", def: "deck" }])}>add</button>
    </ul>
  );
}

export function App() {
  return <Deck />;
}
