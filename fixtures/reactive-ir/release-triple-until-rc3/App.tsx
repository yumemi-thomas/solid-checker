// `until` on the solid-js/@solidjs/signals 2.0.0-rc.3 triple, where it does
// not exist: the import below is TS2305 against the published rc.3 typings,
// and neither runtime has the export. SC2005 states rc.5+'s dev observer guard
// ("Cannot call until inside a reactive scope"), a claim these bytes cannot
// make true, so it must not fire here. The same source on rc.9 is
// `rc9-until-scope`'s first positive.
import { createMemo, createSignal, until } from "solid-js";

const [ready] = createSignal(false);

export function InMemoCompute() {
  const label = createMemo(() => {
    void until(() => ready());
    return ready();
  });
  return <div>{String(label())}</div>;
}
