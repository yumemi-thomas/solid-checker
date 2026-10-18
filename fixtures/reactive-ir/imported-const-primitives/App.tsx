import {
  ALIASED,
  ASSERTED,
  CALLED,
  CHAINED,
  CLUSTER,
  LIMIT,
  MUTABLE,
  SETTINGS,
} from "./constants";
import DEFAULT from "./constants";
import { VIA_BARREL } from "./barrel";
import { CLUSTER as VIA_STAR } from "./star";

// ADR 0325: a named import of another module's exported `const` holds a
// primitive when its initializer is proven one, and settles the unmodeled use
// a callee makes of the prop or argument it feeds. Each component below has
// exactly one tag.

declare function lookup(value: unknown): number;
function fetchThing(id: unknown) {
  return lookup(id);
}

function ClusterScreen(props: { id: string }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Cluster() {
  return <ClusterScreen id={CLUSTER} />;
}
function LimitScreen(props: { id: number }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Limit() {
  return <LimitScreen id={LIMIT} />;
}
function AliasedScreen(props: { id: string }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Aliased() {
  return <AliasedScreen id={ALIASED} />;
}
// The argument itself, with no component between.
export function Direct() {
  const value = fetchThing(CLUSTER);
  return <div>{value}</div>;
}

// Refusals.
// A constant initialized from another exported constant of its own module: the
// file-local proof refuses an exported constant, so the chain is not followed.
function ChainedScreen(props: { id: string }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Chained() {
  return <ChainedScreen id={CHAINED} />;
}
function ObjectScreen(props: { id: unknown }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Object_() {
  return <ObjectScreen id={SETTINGS} />;
}
function CalledScreen(props: { id: string }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Called() {
  return <CalledScreen id={CALLED} />;
}
function AssertedScreen(props: { id: string }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Asserted() {
  return <AssertedScreen id={ASSERTED} />;
}
function MutableScreen(props: { id: string }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Mutable() {
  return <MutableScreen id={MUTABLE} />;
}
function DefaultScreen(props: { id: string }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Default() {
  return <DefaultScreen id={DEFAULT} />;
}
function BarrelScreen(props: { id: string }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Barrel() {
  return <BarrelScreen id={VIA_BARREL} />;
}
function StarScreen(props: { id: string }) {
  const value = fetchThing(props.id);
  return <div>{value}</div>;
}
export function Star() {
  return <StarScreen id={VIA_STAR} />;
}
