import { createSignal } from "solid-js";
import { useQuery, useWindowQuery } from "dep-query";

// Violation: the folded handler forwards every read to the store.
export function FoldedQuery() {
  const query = useQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}

// No proof: the `typeof window` branch is live.
export function WindowQuery() {
  const query = useWindowQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
