import { createSignal, For, Show } from "solid-js";

const [count] = createSignal(1);
const items: { id: number; name: string }[] = [];

export function Bad() {
  return <Show when={count()}>{() => {
    const frozen = count();
    return <span>{frozen}</span>;
  }}</Show>;
}

export function Good() {
  return <Show when={count()}>{() => <span>{count()}</span>}</Show>;
}

export function ParameterReads() {
  return <Show when={count()}>{value => {
    const frozen = value();
    return <span>{frozen}</span>;
  }}</Show>;
}

export function ParameterTracked() {
  return <Show when={count()}>{value => <span>{value()}</span>}</Show>;
}

// A dynamic boolean `keyed` picks the keyed or unkeyed overload at runtime,
// so the callback's shape is ambiguous (RFC 03: "prefer a literal `true`,
// literal `false`, or a custom key function"). The dialect claims *nothing*
// for either parameter rather than fabricate an accessor for what may be a
// raw value — neither usage below may produce an accessor-based finding.
export function DynamicKeyedRawUsage() {
  const [flag] = createSignal(true);
  return <For each={items} keyed={flag()}>{(item, index) => {
    const name = item.name;
    const position = index;
    return <span>{position}: {name}</span>;
  }}</For>;
}

export function DynamicKeyedAccessorUsage() {
  const [flag] = createSignal(false);
  return <For each={items} keyed={flag()}>{(item, index) => {
    const name = item().name;
    const position = index();
    return <span>{position}: {name}</span>;
  }}</For>;
}

// A named key *function* is provable through type facts, so the custom-key
// overload still claims both parameters as accessors: the frozen setup-time
// reads below stay reported.
const byId = (item: { id: number; name: string }) => item.id;

export function ProvenKeyFunction() {
  return <For each={items} keyed={byId}>{(item, index) => {
    const frozen = item().name;
    const position = index();
    return <span>{position}: {frozen}</span>;
  }}</For>;
}
