import { createMemo, createSignal, resolve, until } from "solid-js";

const [ready] = createSignal(false);

// One tracked compute holds both defects: `resolve` is SC2004 and `until` is
// SC2005, under the one rule name no-await-primitive-in-tracked-scope. This
// project has no rule-options; each variant disables a former or the current
// name and keeps what that name covers.
export function Both() {
  const label = createMemo(() => {
    void resolve(() => ready());
    void until(() => ready());
    return ready();
  });
  return <div>{String(label())}</div>;
}
