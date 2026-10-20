import { CLUSTER } from "./constants";
import { CLUSTER as FROM_CJS } from "./script.cjs";
import { CLUSTER as FROM_EQUALS } from "./equals";
import { CLUSTER as FROM_MIXED } from "./mixed";
import { CLUSTER as FROM_MIXED_EXPORTS } from "./mixed-exports";

// ADR 0325 refusals for CommonJS: a module the CommonJS system defines the
// exports of is not a frozen ES module namespace. `Cluster` is the settled
// control; each other component is refused and keeps its `SC9012`.

declare function lookup(value: unknown): number;
function fetchThing(id: unknown) {
  return lookup(id);
}

function Screen(props: { id: unknown }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Cluster() {
  return <Screen id={CLUSTER} />;
}
function CjsScreen(props: { id: unknown }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Cjs() {
  return <CjsScreen id={FROM_CJS} />;
}
function EqualsScreen(props: { id: unknown }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Equals() {
  return <EqualsScreen id={FROM_EQUALS} />;
}
function MixedScreen(props: { id: unknown }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Mixed() {
  return <MixedScreen id={FROM_MIXED} />;
}
function MixedExportsScreen(props: { id: unknown }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function MixedExports() {
  return <MixedExportsScreen id={FROM_MIXED_EXPORTS} />;
}
