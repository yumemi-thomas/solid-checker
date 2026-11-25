import { createOptimistic, untrack } from "solid-js";

// ADR 1216 slice 3: a Map/Set seeded from a literal runs its forEach callback; a function stored once in a fresh local
// collection runs where the collection hands it back and calls it. OPTIMISTIC_WRITE_BEFORE_LOAD on rc.21.

// Violations.
export function SeededMap() {
  const [value, setValue] = createOptimistic(async () => 1);
  const run = () => {
    setValue(2);
  };
  new Map([[1, 1]]).forEach(run);
  return <p>{value()}</p>;
}

export function SeededSet() {
  const [value, setValue] = createOptimistic(async () => 1);
  const run = () => {
    setValue(3);
  };
  new Set([1]).forEach(run);
  return <p>{value()}</p>;
}

export function StoredInMap() {
  const [value, setValue] = createOptimistic(async () => 1);
  const table = new Map<number, () => void>();
  const run = () => {
    setValue(4);
  };
  table.set(1, run);
  table.forEach((g) => g());
  return <p>{value()}</p>;
}

export function AddedToSetUnderUntrack() {
  const [value, setValue] = createOptimistic(async () => 1);
  const table = new Set<() => void>();
  untrack(() => {
    const run = () => {
      setValue(5);
    };
    table.add(run);
  });
  untrack(() => {
    table.forEach((g) => g());
  });
  return <p>{value()}</p>;
}

export function StoredInWeakMap() {
  const [value, setValue] = createOptimistic(async () => 1);
  const key = {};
  const table = new WeakMap<object, () => void>();
  const run = () => {
    setValue(6);
  };
  table.set(key, run);
  table.get(key)!();
  return <p>{value()}</p>;
}

// Uncertifiable: no proven run.
export function EmptySet() {
  const [value, setValue] = createOptimistic(async () => 1);
  const run = () => {
    setValue(7);
  };
  new Set<number>().forEach(run);
  return <p>{value()}</p>;
}

export function StoredThenDeleted() {
  const [value, setValue] = createOptimistic(async () => 1);
  const table = new Map<number, () => void>();
  const run = () => {
    setValue(8);
  };
  table.set(1, run);
  table.delete(1);
  table.forEach((g) => g());
  return <p>{value()}</p>;
}

export function StoredNeverRun() {
  const [value, setValue] = createOptimistic(async () => 1);
  const table = new Set<() => void>();
  const run = () => {
    setValue(9);
  };
  table.add(run);
  return <p>{value()}</p>;
}

export function RunBeforeStore() {
  const [value, setValue] = createOptimistic(async () => 1);
  const table = new Set<() => void>();
  const run = () => {
    setValue(10);
  };
  table.forEach((g) => g());
  table.add(run);
  return <p>{value()}</p>;
}

export function OtherKey() {
  const [value, setValue] = createOptimistic(async () => 1);
  const key = {};
  const other = {};
  const table = new WeakMap<object, () => void>();
  const run = () => {
    setValue(11);
  };
  table.set(key, run);
  table.get(other)?.();
  return <p>{value()}</p>;
}

export function OptionalCall() {
  const [value, setValue] = createOptimistic(async () => 1);
  const key = {};
  const table = new WeakMap<object, () => void>();
  const run = () => {
    setValue(12);
  };
  table.set(key, run);
  table.get(key)?.();
  return <p>{value()}</p>;
}
