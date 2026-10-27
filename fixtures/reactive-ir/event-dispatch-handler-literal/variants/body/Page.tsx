import { withTx } from "./tx";

// A call in a component's body runs under the component's owner, so the
// helper's other callers are not all events.
export function Page() {
  withTx(() => undefined);
  return <div />;
}
