// kui feature-messages MessagesRoute.tsx:611 (track.ts:191): a component calls a
// helper without its defaulted parameter; the helper hands the default
// (`new Date()`) to another helper that calls a member on it.
import { createSignal } from "solid-js";

type Query = { from: number; value: string };

function defaultWindow(now: Date): { from: number } {
  return { from: now.getTime() - 3_600_000 };
}

function emptyQuery(now: Date = new Date()): Query {
  return { from: defaultWindow(now).from, value: "" };
}

export function TrackScreen(props: { clusterId: string }) {
  const [query] = createSignal<Query>(emptyQuery());
  return <p>{props.clusterId + query().from}</p>;
}
