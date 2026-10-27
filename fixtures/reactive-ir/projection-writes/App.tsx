import { createProjection, createSignal, createStore } from "solid-js";

const [id] = createSignal("a");

interface Selection {
  list: number[];
  total: number;
  other?: boolean;
  [key: string]: unknown;
}

const selected: Selection = createProjection(
  (draft: Selection) => {
    // Negative: the draft is the projection's own writable view.
    draft[id()] = true;
    draft.total = 1;
    // Negative: the result symbol, inside its own compute function, commits.
    selected.other = true;
    selected.list.push(1);
  },
  { list: [], total: 0 },
);

// Positive: assignment, update and mutator, outside the compute function.
export function Outside() {
  selected.other = true;
  selected.total++;
  selected.list.push(2);
}

// A function-form `createStore` derives its store the same way; the user
// setter is the other way in.
const [derived, setDerived] = createStore(
  (draft: { n: number }) => {
    draft.n = 1;
    derived.n = 2;
  },
  { n: 0 },
);
export function DerivedStore() {
  // Positive: written outside both the setter and the compute function.
  derived.n = 3;
  // Negative: its own setter.
  return <button onClick={() => setDerived(() => { derived.n = 4; })}>go</button>;
}
