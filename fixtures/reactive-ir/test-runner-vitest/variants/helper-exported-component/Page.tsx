import { choose } from "./helpers";

// A call in a component's body runs under the component's owner: the helper's
// dispatch can deliver a handler there, so the veto stays.
export function Page() {
  choose(document.createElement("input"));
  return <div />;
}
