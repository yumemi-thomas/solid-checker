// Violation. Corpus: app-game packages/spector/solid/spector-result-view.tsx:796 and :819. A component body runs
// once, so a return that branches on a prop keeps the first branch. Both corpus sites are latent: no in-repo
// caller passes onReadMesh or onReadScene, so the status never flips there. But SpectorResultView is a public
// export of @app-game/spector ("./solid/result-view"), and an rc.17 runtime probe where a caller drops onReadMesh
// while an available mesh is shown keeps MeshResult on screen ("mesh:undefined" instead of the unavailable
// reason): a real defect under the exported-is-reachable premise.
import { createSignal } from "solid-js";

type Result = { status: "available" | "missing"; reason: string };

function MeshResult(props: { result: Result }) {
  return props.result.status === "available" ? <b>mesh</b> : <i>{props.result.reason}</i>;
}

export function App() {
  const [result] = createSignal<Result>({ status: "available", reason: "" });
  return <MeshResult result={result()} />;
}
