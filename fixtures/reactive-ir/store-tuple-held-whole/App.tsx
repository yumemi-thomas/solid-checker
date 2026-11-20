import { createStore } from "solid-js";

// Positive: `st[0]` is the store proxy, so `.v` is its strict-read Get (STRICT_READ_UNTRACKED in Chrome).
export function ElementGet() {
  const st = createStore({ v: 1 });
  void st[0].v;
  return <p />;
}

// Positive, through an alias of the tuple.
export function AliasElementGet() {
  const st = createStore({ v: 1 });
  const pair = st;
  void pair[0].v;
  return <p />;
}

// Control: the destructured store's Get.
export function SlotGet() {
  const [s] = createStore({ v: 1 });
  void s.v;
  return <p />;
}

// Negative: an index of the tuple is no store Get (Chrome silent). What the element is handed to is not
// followed: uncertifiable, never a violation.
export function ElementHandedOn() {
  const st = createStore({ v: 1 });
  const o = st[0];
  return <p>{o.v}</p>;
}

// Negative: the setter slot and the element itself; the tracked JSX Get stays clean.
export function SetterAndElement() {
  const st = createStore({ v: 1 });
  const set = st[1];
  void st[0];
  return <button onClick={() => set((d) => { d.v = 2; })}>{st[0].v}</button>;
}
