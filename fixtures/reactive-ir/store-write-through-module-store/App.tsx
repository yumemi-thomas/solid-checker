import { called, direct, kept, setCalled, viaAlias } from "./stores";

// ADR 1232: an exported module store whose every reference in another file is the root of a contained use (here, or through a
// local carrier), its setter referenced nowhere else, keeps its own literal (ADR 0491): a write past its root is dropped outside the
// setter (STORE_WRITE_OUTSIDE_SETTER).

// Violations.
export function Direct() {
  direct.a.v = 7;
  return <i />;
}

export function Alias() {
  const held = viaAlias;
  held.items.push(1);
  return <i />;
}

// Uncertifiable.
declare function keep(value: unknown): void;

export function Kept() {
  keep(kept);
  kept.a.v = 7;
  return <i />;
}

export function SetterElsewhere() {
  const reset = () =>
    setCalled((draft) => {
      draft.a.v = 0;
    });
  void reset;
  called.a.v = 7;
  return <i />;
}
