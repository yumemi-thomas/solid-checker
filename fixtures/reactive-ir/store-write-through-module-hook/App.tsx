import { useHanded, useKept, useSetter, useVals } from "./hooks";

// ADR 1232: a store a hook of another module creates, every call's value used only as the root of contained uses, keeps its own
// literal (ADR 0491): a write past its root is dropped outside the setter (STORE_WRITE_OUTSIDE_SETTER).

// Violations.
export function Destructured() {
  const { s1 } = useVals();
  s1.a.v = 7;
  return <i />;
}

export function Member() {
  const vals = useVals();
  vals.s1.a.v = 7;
  return <i />;
}

const shared = useVals();
export function Shared() {
  shared.s1.items.push(1);
  return <i />;
}

// Uncertifiable.
declare function keep(value: unknown): void;

export function Kept() {
  const vals = useKept();
  keep(vals);
  vals.s1.a.v = 7;
  return <i />;
}

export function SetterRead() {
  const { s1, setS1 } = useSetter();
  setS1((draft) => {
    draft.a.v = 1;
  });
  s1.a.v = 7;
  return <i />;
}

export function HookHanded() {
  keep(useHanded);
  const vals = useHanded();
  vals.s1.a.v = 7;
  return <i />;
}
