// Violation. Corpus: ai-memory-ui src/screens/graph.tsx:269 and workspace-cascader.tsx:189,
// probus-hk Search.tsx:1296: a For map function runs untracked, so a signal or memo read in its
// body (outside the JSX it returns) is a snapshot.
import { createMemo, createSignal, For } from "solid-js";

function Graph(props: { links: { from: string; to: string }[] }) {
  const [names] = createSignal(new Map<string, number>());
  const reversed = createMemo(() => new Set(props.links.map((link) => link.to)));
  return (
    <For each={props.links}>
      {(link) => {
        const index = names().get(link.from);
        const back = reversed().has(link.to);
        return <i data-back={back}>{index}</i>;
      }}
    </For>
  );
}

export function App() {
  const [links] = createSignal([{ from: "a", to: "b" }]);
  return <Graph links={links()} />;
}
