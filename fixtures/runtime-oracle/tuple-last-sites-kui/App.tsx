import { createEffect, createSignal } from "solid-js";
import { createHealth } from "./health";

// Site probes extracted from the frozen kui corpus. Names map each original SC1001 site.
function TopicListSeed(props: { query: { search: string } }) {
  // TopicListPage.tsx:298
  const [typed] = createSignal(props.query.search);
  return <p>{typed()}</p>;
}
export function TopicListSeedHost() {
  const [query] = createSignal({ search: "a" });
  return <TopicListSeed query={query()} />;
}

export function TopicListApply() {
  const [typed, setTyped] = createSignal("a");
  const [source] = createSignal("b");
  createEffect(() => source(), (search: string) => {
    // TopicListPage.tsx:314
    if (search !== typed().trim()) setTyped(search);
  });
  return <p>{typed()}</p>;
}

export function TopicsRouteSearch() {
  const [query, changeQuery] = createSignal({ search: "a", facet: "all" });
  const [source] = createSignal({ search: "b", facet: "all" });
  createEffect(() => source(), (asked) => {
    // TopicsRoute.tsx:522 column 28: the first operand is true.
    if (asked.search !== query().search || asked.facet !== query().facet) changeQuery(asked);
  });
  return <p>{query().search}</p>;
}

export function TopicsRouteFacet() {
  const [query, changeQuery] = createSignal({ search: "a", facet: "all" });
  const [source] = createSignal({ search: "a", facet: "some" });
  createEffect(() => source(), (asked) => {
    // TopicsRoute.tsx:522 column 62: equal search values force evaluation of the second operand.
    if (asked.search !== query().search || asked.facet !== query().facet) changeQuery(asked);
  });
  return <p>{query().facet}</p>;
}

export function ShellHealth() {
  // App.tsx:154; health.ts is copied verbatim from the frozen corpus.
  const health = createHealth({
    now: () => new Date(),
    schedule: (delayMs, run) => void setTimeout(run, delayMs),
    onRetry: () => { void 0; },
  });
  return <p>{health.connectivity().kind}</p>;
}

export function ShellSelected() {
  const [selected, select] = createSignal("old");
  const cluster = { selected, select };
  const [route] = createSignal<string | undefined>("new");
  createEffect(() => route(), (named) => {
    // App.tsx:836 (standard only).
    if (named !== undefined && named !== cluster.selected()) cluster.select(named);
  });
  return <p>{selected()}</p>;
}
