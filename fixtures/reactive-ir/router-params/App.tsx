import { Show } from "solid-js";
import { useParams, useSearchParams } from "@solidjs/router";
import * as R from "@solidjs/router";

// ADR 0324: `useParams()` of the audited `@solidjs/router` release hands out
// route parameters as URL path-segment strings (`undefined` when an optional
// one is absent). A `<Show when={params.k}>` callback therefore receives a
// string, which settles the unmodeled use a callee makes of it. Every
// component below is called from exactly one `<Show>`, so the value domain of
// its prop is the router's.

declare function lookup(value: unknown): number;
function fetchThing(id: string) {
  return lookup(id);
}
function Screen(props: { clusterId: string }) {
  const value = fetchThing(props.clusterId);
  return <div>{value}</div>;
}

// Settled: the accessor form and the keyed form.
export function RouteAccessor() {
  const params = useParams<{ clusterId?: string }>();
  return (
    <Show when={params.clusterId}>{(id) => <Screen clusterId={id()} />}</Show>
  );
}
function KeyedScreen(props: { clusterId: string }) {
  const value = fetchThing(props.clusterId);
  return <div>{value}</div>;
}
export function RouteKeyed() {
  const params = useParams<{ clusterId?: string }>();
  return (
    <Show when={params.clusterId} keyed>
      {(id) => <KeyedScreen clusterId={id} />}
    </Show>
  );
}

// Refusals, each the premise's own boundary. The prop is the same string
// domain in every case; what is missing is the proof that it is the router's.
function ComputedScreen(props: { value: unknown }) {
  const value = fetchThing(props.value as string);
  return <div>{value}</div>;
}
export function ComputedKey(props: { key: string }) {
  const params = useParams<Record<string, string | undefined>>();
  return (
    <Show when={params[props.key]}>
      {(value) => <ComputedScreen value={value()} />}
    </Show>
  );
}
function ProtoScreen(props: { value: unknown }) {
  const value = fetchThing(props.value as string);
  return <div>{value}</div>;
}
export function PrototypeName() {
  const params = useParams<{ toString?: string }>();
  return (
    <Show when={params.toString}>
      {(value) => <ProtoScreen value={value()} />}
    </Show>
  );
}
function AliasScreen(props: { clusterId: string }) {
  const value = fetchThing(props.clusterId);
  return <div>{value}</div>;
}
export function AliasedParams() {
  const params = useParams<{ clusterId?: string }>();
  const alias = params;
  return (
    <Show when={alias.clusterId}>
      {(id) => <AliasScreen clusterId={id()} />}
    </Show>
  );
}
function useMyParams() {
  return useParams<{ clusterId?: string }>();
}
function WrappedScreen(props: { clusterId: string }) {
  const value = fetchThing(props.clusterId);
  return <div>{value}</div>;
}
export function WrappedUseParams() {
  const params = useMyParams();
  return (
    <Show when={params.clusterId}>
      {(id) => <WrappedScreen clusterId={id()} />}
    </Show>
  );
}
function NamespaceScreen(props: { clusterId: string }) {
  const value = fetchThing(props.clusterId);
  return <div>{value}</div>;
}
export function NamespaceUseParams() {
  const params = R.useParams<{ clusterId?: string }>();
  return (
    <Show when={params.clusterId}>
      {(id) => <NamespaceScreen clusterId={id()} />}
    </Show>
  );
}
function ReassignedScreen(props: { clusterId: string }) {
  const value = fetchThing(props.clusterId);
  return <div>{value}</div>;
}
export function ReassignedParams() {
  let params = useParams<{ clusterId?: string }>();
  params = { clusterId: "fixed" };
  return (
    <Show when={params.clusterId}>
      {(id) => <ReassignedScreen clusterId={id()} />}
    </Show>
  );
}
function SearchScreen(props: { range: string }) {
  const value = fetchThing(props.range);
  return <div>{value}</div>;
}
export function SearchParams() {
  const [search] = useSearchParams<{ range?: string }>();
  return (
    <Show when={search.range}>{(range) => <SearchScreen range={range()} />}</Show>
  );
}
function DynamicKeyedScreen(props: { clusterId: string }) {
  const value = fetchThing(props.clusterId);
  return <div>{value}</div>;
}
export function DynamicKeyed(props: { flag: boolean }) {
  const params = useParams<{ clusterId?: string }>();
  return (
    <Show when={params.clusterId} keyed={props.flag as false}>
      {(id) => <DynamicKeyedScreen clusterId={id()} />}
    </Show>
  );
}
function WrittenScreen(props: { shard: string }) {
  const value = fetchThing(props.shard);
  return <div>{value}</div>;
}
// A member of that name is written somewhere in the program.
const bag: { shard?: string } = {};
bag.shard = "a";
export function WrittenName() {
  const params = useParams<{ shard?: string }>();
  return (
    <Show when={params.shard}>{(id) => <WrittenScreen shard={id()} />}</Show>
  );
}
function DefinedScreen(props: { tenant: string }) {
  const value = fetchThing(props.tenant);
  return <div>{value}</div>;
}
// A member of that name is defined reflectively.
Object.defineProperty(bag, "tenant", { value: "t" });
export function DefinedName() {
  const params = useParams<{ tenant?: string }>();
  return (
    <Show when={params.tenant}>{(id) => <DefinedScreen tenant={id()} />}</Show>
  );
}
