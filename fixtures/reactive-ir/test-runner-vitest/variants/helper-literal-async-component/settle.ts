import { fireAndFlush } from "./helpers";

const node = document.createElement("input");

// Exported: a component's body calls it too, so the part before its await
// runs under that component's owner, and the dispatch keeps the veto.
export async function settleThenFire() {
  await Promise.resolve();
  fireAndFlush(() => node.dispatchEvent(new Event("settle")));
}
