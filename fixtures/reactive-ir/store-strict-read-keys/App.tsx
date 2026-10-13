import { createStore } from "solid-js";
import { useForwardedStore } from "./hook";

const userKey = Symbol("user");
declare const dynamicKey: "data" | "then";

// Violations: a Get of a known string key other than "then" warns
// STRICT_READ_UNTRACKED in a component body, function-valued members included.
export function OwnFunction() {
  const [q] = createStore({ refetch: () => {}, data: 1 });
  q.refetch();
  return <p>ok</p>;
}
export function DataMember() {
  const [q] = createStore({ data: 1 });
  const value = q.data;
  return <p>{value}</p>;
}
export function KnownComputedKey() {
  const [q] = createStore({ data: 1 });
  const value = q["data"];
  return <p>{value}</p>;
}
export function DestructuredData() {
  const [q] = createStore({ then: () => {}, data: 1 });
  const { data } = q;
  return <p>{data}</p>;
}
export function AliasedData() {
  const [q] = createStore({ then: () => {}, data: 1 });
  const alias = q;
  const value = alias.data;
  return <p>{value}</p>;
}
export function ForwardedFunction() {
  const q = useForwardedStore();
  q.refetch();
  return <p>ok</p>;
}

// No violation: the store Get of "then" or of a symbol key does not warn.
export function ThenRead() {
  const [q] = createStore({ then: () => {}, data: 1 });
  q.then();
  return <p>ok</p>;
}
export function ComputedThen() {
  const [q] = createStore({ then: () => {}, data: 1 });
  q["then"]();
  return <p>ok</p>;
}
export function SymbolRead() {
  const [q] = createStore({ [userKey]: 1, data: 1 });
  const value = q[userKey];
  return <p>{value}</p>;
}
export function DestructuredThen() {
  const [q] = createStore({ then: () => {}, data: 1 });
  const { then } = q;
  then();
  return <p>ok</p>;
}
export function AliasedThen() {
  const [q] = createStore({ then: () => {}, data: 1 });
  const alias = q;
  alias.then();
  return <p>ok</p>;
}
export function ForwardedThen() {
  const q = useForwardedStore();
  q.then();
  return <p>ok</p>;
}

// Uncertifiable: the key may be "then", whose Get does not warn.
export function UnknownComputedKey() {
  const [q] = createStore({ then: () => {}, data: 1 });
  const value = q[dynamicKey];
  return <p>{String(value)}</p>;
}

// Chains: the head Get decides. A warning head is a violation whatever
// follows; an exempt head followed by another Get is not decided here.
export function NestedThen() {
  const [q] = createStore({ nested: { then: () => {}, value: 1 } });
  q.nested.then();
  return <p>ok</p>;
}
export function ThenHeadChain() {
  const [q] = createStore({ then: { value: 1 }, data: 1 });
  const value = q.then.value;
  return <p>{value}</p>;
}
export function SymbolHeadChain() {
  const [q] = createStore({ [userKey]: { value: 1 }, data: 1 });
  const value = q[userKey].value;
  return <p>{value}</p>;
}

// Through a project helper: the helper's Get is the read.
type Remote = { then: () => void; data: number };
function callThen(store: Remote) {
  store.then();
}
function readData(store: Remote) {
  return store.data;
}
export function HelperThen() {
  const [q] = createStore<Remote>({ then: () => {}, data: 1 });
  callThen(q);
  return <p>ok</p>;
}
export function HelperData() {
  const [q] = createStore<Remote>({ then: () => {}, data: 1 });
  const value = readData(q);
  return <p>{value}</p>;
}

// Through a prop: the child's Get of the store it was handed.
function ThenChild(props: { store: Remote }) {
  props.store.then();
  return <p>ok</p>;
}
function DataChild(props: { store: Remote }) {
  const value = props.store.data;
  return <p>{value}</p>;
}
export function PropParent() {
  const [q] = createStore<Remote>({ then: () => {}, data: 1 });
  return (
    <main>
      <ThenChild store={q} />
      <DataChild store={q} />
    </main>
  );
}

// Through a local closure the body calls: the closure's Get is the read.
export function ClosureThen() {
  const [q] = createStore({ then: () => {}, data: 1 });
  const call = () => q.then();
  call();
  return <p>ok</p>;
}
export function ClosureData() {
  const [q] = createStore({ then: () => {}, data: 1 });
  const read = () => q.data;
  const value = read();
  return <p>{value}</p>;
}
