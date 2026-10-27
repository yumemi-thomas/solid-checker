import { count, setCount } from "./Card";
import { fireAndFlush } from "./helpers";

// The same helper called in a component's body runs its literal under the
// component's owner: the write is SC2001 and the read SC1001, and the helper
// is no longer test-only, so the dispatch literal in the test keeps the veto.
export function Page() {
  fireAndFlush(() => setCount(count() + 1));
  return <div />;
}
