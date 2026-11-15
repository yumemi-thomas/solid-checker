import { createOptimistic, createRoot } from "solid-js";

// SC2009 (ADR 0987): an optimistic setter of an async source throws OPTIMISTIC_WRITE_BEFORE_LOAD before the first answer, in every form
// that hands the setter out: the declaration, an assignment's destructuring, a root's callback result, a project factory's result.
// The `Sync*` twins have an answer already (a data value): no throw.

export function Declared() {
  const [list, setList] = createOptimistic(async () => 1);
  setList(2);
  return <p>{list()}</p>;
}

export function Assigned() {
  let list: () => number;
  let setList: (v: number) => void;
  [list, setList] = createOptimistic(async () => 1);
  setList(2);
  return <p>{list()}</p>;
}

export function ViaRoot() {
  const [list, setList] = createRoot(() => createOptimistic(async () => 1));
  setList(2);
  return <p>{list()}</p>;
}

function makeAsync() {
  return createOptimistic(async () => 1);
}

export function ViaFactory() {
  const [list, setList] = makeAsync();
  setList(2);
  return <p>{list()}</p>;
}

export function SyncDeclared() {
  const [list, setList] = createOptimistic(1);
  setList(2);
  return <p>{list()}</p>;
}

export function SyncAssigned() {
  let list: () => number;
  let setList: (v: number) => void;
  [list, setList] = createOptimistic(1);
  setList(2);
  return <p>{list()}</p>;
}

export function SyncViaRoot() {
  const [list, setList] = createRoot(() => createOptimistic(1));
  setList(2);
  return <p>{list()}</p>;
}

function makeSync() {
  return createOptimistic(1);
}

export function SyncViaFactory() {
  const [list, setList] = makeSync();
  setList(2);
  return <p>{list()}</p>;
}
