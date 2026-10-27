// kui feature-topics TopicListPage.tsx:320,654 (callback from TopicsRoute.tsx:631) and app-game abr-viewer BrushDetailEditable.tsx:74-81: a child
// calls its callback prop from two arrows given to the same child whose handling of its own callback is unresolved (merge view, nested helper).
import { createSignal, merge } from "solid-js";

function Select(props: { readonly size?: "sm" | "md"; readonly onChange?: (value: string) => void }) {
  const p = merge({ size: "md" } as const, props);
  function commit(value: string): void {
    props.onChange?.(value);
  }
  return <button class={p.size} onClick={() => commit("a")} />;
}

function ListPage(props: { readonly onQueryChange: (query: string) => void }) {
  return (
    <>
      <Select onChange={(next) => props.onQueryChange(next)} />
      <Select size="sm" onChange={(next) => props.onQueryChange(`${next}!`)} />
    </>
  );
}

export function Route() {
  const [query, setQuery] = createSignal("");
  const changeQuery = (next: string): void => {
    setQuery(next);
  };
  return (
    <>
      <ListPage onQueryChange={changeQuery} />
      <p>{query()}</p>
    </>
  );
}
