import { createStore } from "solid-js";

// ADR 1169 (R3 G15): an exact `const` alias of a store link (`const held = s1.a`) holds that link's proxy when ADR 0491's proof
// holds for the store (its own literal, the setter never referenced, every use contained) and the literal holds an object there:
// a write through the alias is dropped outside the setter (SC2003), a Get through it is the store's Get (SC1001).

function keep(value: unknown) {
  return value;
}

// Violations.
function IifeWrite() {
  const [s1] = createStore({ a: { v: 0 } });
  (() => {
    const held = s1.a;
    held.v = 7;
  })();
  return <i />;
}

function TimerWrite() {
  const [s1] = createStore({ a: { v: 0 } });
  setTimeout(() => {
    const held = s1.a;
    held.v = 8;
  }, 0);
  return <i />;
}

function DeepWrite() {
  const [s1] = createStore({ a: { b: { v: 0 } } });
  const held = s1.a;
  held.b.v = 9;
  return <i />;
}

function AliasRead() {
  const [s1] = createStore({ a: { v: 0 } });
  const held = s1.a;
  void held.v;
  return <i />;
}

// Uncertifiable, as before.
// The setter is referenced: the link may be replaced.
function SetterReferenced() {
  const [s1, setS1] = createStore({ a: { v: 0 } });
  const held = s1.a;
  held.v = 7;
  return <button onClick={() => setS1((draft) => { draft.a = { v: 1 }; })}>go</button>;
}

// The alias is handed on.
function HandedOn() {
  const [s1] = createStore({ a: { v: 0 } });
  const held = s1.a;
  keep(held);
  held.v = 7;
  return <i />;
}

// A `let` alias may be reassigned.
function LetAlias() {
  const [s1] = createStore({ a: { v: 0 } });
  let held = s1.a;
  held.v = 7;
  return <i />;
}

// An array link (no object literal at the key).
function ArrayAlias() {
  const [s1] = createStore({ items: [0] });
  const held = s1.items;
  held.push(1);
  return <i />;
}

export function Page() {
  return (
    <div>
      <IifeWrite />
      <TimerWrite />
      <DeepWrite />
      <AliasRead />
      <SetterReferenced />
      <HandedOn />
      <LetAlias />
      <ArrayAlias />
    </div>
  );
}
