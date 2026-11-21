import * as S from "solid-js";
import { createStore } from "solid-js";
import type { StoreSetter } from "solid-js";

// ADR 1172: a store handed by prop to an exact local component that reads the prop only as the root of a contained use (and the
// setter, if handed, never read) still holds its own literal (ADR 0491): a write past the root through the prop is dropped outside
// the setter (STORE_WRITE_OUTSIDE_SETTER).

type Items = { a: { v: number }; items: number[] };
type Props = { s1: Items; setS1: StoreSetter<Items> };

// Violations.
function AssignChild(props: Props) {
  props.s1.a.v = 7;
  return <i />;
}
function AssignParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <AssignChild s1={s1} setS1={setS1} />;
}

function PushChild({ s1 }: { s1: Items }) {
  s1.items.push(1);
  return <i />;
}
function PushParent() {
  const [s1] = createStore({ a: { v: 0 }, items: [0] });
  return <PushChild s1={s1} />;
}

function NamespaceChild(props: { s1: Items }) {
  props.s1.a.v = 8;
  return <i />;
}
function NamespaceParent() {
  const [s1] = S.createStore({ a: { v: 0 }, items: [0] });
  return <NamespaceChild s1={s1} />;
}

// The prop held in a `const` alias first.
function AliasChild(props: { s1: Items }) {
  const held = props.s1;
  held.a.v = 9;
  return <i />;
}
function AliasParent() {
  const [s1] = createStore({ a: { v: 0 }, items: [0] });
  return <AliasChild s1={s1} />;
}

// Uncertifiable, as before.
// The setter is read and called (in a handler): a link may hold a value the literal did not write.
function CallsSetterChild(props: Props) {
  props.s1.a.v = 7;
  return <button onClick={() => props.setS1((draft) => { draft.a = { v: 1 }; })} />;
}
function CallsSetterParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <CallsSetterChild s1={s1} setS1={setS1} />;
}

// The prop's value leaves: handed to a function.
function keep(value: unknown) {
  return value;
}
function HandsOnChild(props: { s1: Items }) {
  keep(props.s1.a);
  props.s1.a.v = 7;
  return <i />;
}
function HandsOnParent() {
  const [s1] = createStore({ a: { v: 0 }, items: [0] });
  return <HandsOnChild s1={s1} />;
}

// The props object is used whole.
function WholeChild(props: { s1: Items }) {
  keep(props);
  props.s1.a.v = 7;
  return <i />;
}
function WholeParent() {
  const [s1] = createStore({ a: { v: 0 }, items: [0] });
  return <WholeChild s1={s1} />;
}

// The store also leaves the parent as a value.
function AlsoHandedChild(props: { s1: Items }) {
  props.s1.a.v = 7;
  return <i />;
}
function AlsoHandedParent() {
  const [s1] = createStore({ a: { v: 0 }, items: [0] });
  keep(s1);
  return <AlsoHandedChild s1={s1} />;
}

export function Page() {
  return (
    <div>
      <AssignParent />
      <PushParent />
      <NamespaceParent />
      <AliasParent />
      <CallsSetterParent />
      <HandsOnParent />
      <WholeParent />
      <AlsoHandedParent />
    </div>
  );
}
