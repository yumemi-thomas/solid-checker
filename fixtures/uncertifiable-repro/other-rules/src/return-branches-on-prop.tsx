// Violation. Corpus: app-game packages/spector/solid/spector-result-view.tsx:796 and :819: a
// component body runs once, so a return that branches on a prop keeps the first branch.
import { createSignal } from "solid-js";

type Result = { status: "available" | "missing"; reason: string };

function MeshResult(props: { result: Result }) {
  return props.result.status === "available" ? <b>mesh</b> : <i>{props.result.reason}</i>;
}

export function App() {
  const [result] = createSignal<Result>({ status: "available", reason: "" });
  return <MeshResult result={result()} />;
}
