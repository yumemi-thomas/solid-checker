// Corpus adjudication 2026-10-23 (scripts/corpus/reviewed-findings.json): kui
// ConnectRoute.tsx:103 and KsqlRoute.tsx:164,170 spread a conditional on an
// optional prop into an options object. The checker reports the spread as a
// "props spread" read. No caller passes `queries` as a getter: the product
// passes only `clusterId`, the test a plain registry. So the spread reads no
// signal, and the dev build stays quiet: a false positive (static-only).
import { createSignal } from "solid-js";

type Registry = { readonly name: string };

function useQuery(options: { key: () => string; registry?: Registry }) {
  return () => `${options.key()}${options.registry ? ` in ${options.registry.name}` : ""}`;
}

function Screen(props: { clusterId: string; queries?: Registry }) {
  const listing = useQuery({
    key: () => `connect:${props.clusterId}`,
    ...(props.queries === undefined ? {} : { registry: props.queries }),
  });
  return <p data-probe={props.queries ? "test-listing" : "listing"}>{listing()}</p>;
}

// The product route: a signal-backed `clusterId`, no `queries`.
export function ProductRoute() {
  const [clusterId, setClusterId] = createSignal("c1");
  return (
    <section>
      <button type="button" data-probe="switch" onClick={() => setClusterId("c9")}>switch</button>
      <Screen clusterId={clusterId()} />
    </section>
  );
}

// The test mount: a plain registry value.
export function TestMount() {
  const queries: Registry = { name: "test" };
  return <Screen clusterId="c2" queries={queries} />;
}
