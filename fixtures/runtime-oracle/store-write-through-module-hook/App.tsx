import { createStore } from "solid-js";
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
const kept: unknown[] = [];
function keep(value: unknown): void {
  kept.push(value);
}

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

// `local-alias-member-reads`' two same-file local-hook writes (ADR 1232 moves them to violations).
export function StoreMemberWrite() {
  const [state, setState] = createStore({ v: 0, items: [0] });
  const useState = () => ({ state, setState });
  const holder = useState();
  holder.state.v = 1;
  holder.state.items.push(1);
  return <div>{state.v}</div>;
}

export function StoreLinkInClosureThroughMember() {
  const [state, setState] = createStore({ v: 0, nested: { v: 0 } });
  const useState = () => ({ state, setState });
  const holder = useState();
  const write = () => {
    holder.state.nested.v = 1;
  };
  write();
  return <div>{state.v}</div>;
}
