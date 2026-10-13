import { createSignal } from "solid-js";
import { useArrayQuery } from "dep-array";
import { useHoistedQuery, useTdzQuery } from "dep-query";
import { useSelfQuery } from "dep-self";

// Review round 1 of ADR 0295: four folds that changed what the package runs.

// No proof: the trap reads its `const` alias before the declaration runs.
export function TdzQuery() {
  const query = useTdzQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}

// No proof: the dead arm's `var Reflect` is the trap's own binding.
export function HoistedQuery() {
  const query = useHoistedQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}

// No proof: an `exports` array in `dep-bridge` hides an import cycle.
export function ArrayCycleQuery() {
  const query = useArrayQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}

// No proof: `dep-self` imports itself by its own name.
export function SelfQuery() {
  const query = useSelfQuery({ search: "a" });
  const [config] = createSignal(query.search);
  return <main>{config()}</main>;
}
