import { createEffect, createSignal } from "solid-js";
import { put } from "./put";

// ADR 1040 (b), the corpus shapes it regained (kui feature-topics: TopicListPage.tsx:298 and :314, TopicsRoute.tsx:522):
// the window reaches only seen code and the program's one open write is a helper's `o[k] = v`, so the slots of
// `createSignal` stay exact and these reads are the untracked reads they look like (uncertifiable,
// `tuple-iteration-written`, before).
window.addEventListener("popstate", () => console.log("moved"));
const timer = window.setTimeout(() => console.log("later"), 0);
window.clearTimeout(timer);
put({}, "a", 1);

function TopicList(props: { query: { search: string } }) {
  // A prop read in the component body, where nothing tracks.
  const [typed, setTyped] = createSignal(props.query.search);
  createEffect(
    () => props.query.search,
    (search: string) => {
      // A signal read in an effect's apply callback, where nothing tracks.
      if (search !== typed().trim()) setTyped(search);
    },
  );
  return <p>{typed()}</p>;
}

export function TopicsScreen() {
  const [query] = createSignal({ search: "kafka" });
  return <TopicList query={query()} />;
}

export function TopicsRoute() {
  const [query, setQuery] = createSignal({ search: "", facet: "" });
  const [location] = createSignal({ search: "?search=a" });
  createEffect(
    () => location().search,
    (search: string) => {
      const asked = { search: search.slice(8), facet: "" };
      if (asked.search !== query().search || asked.facet !== query().facet) setQuery(asked);
    },
  );
  return <p>{query().search}</p>;
}
