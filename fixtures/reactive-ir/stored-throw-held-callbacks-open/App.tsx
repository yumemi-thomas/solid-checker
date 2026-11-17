import { createContext, createMemo, runWithOwner, useContext } from "solid-js";

// ADR 1044: a function held in a `const` copy, or in a property of a closed object literal (ADR 0351), runs where a use of that
// holder is called or handed off: its stored throw ends where that use's does.
const Missing = createContext<number>(); // no default: a read with no provider throws CONTEXT_NOT_FOUND (SC4002)
const Present = createContext(7); // a default: a read with no owner is NO_OWNER_CONTEXT (SC4001)

// Obligations (unchanged): the holder may be reached or rewritten, the copy may be rebound or chosen, the run caught.
function use(_value: unknown) {}
export function BundleEscapes() {
  const b = { cb: () => { useContext(Present); } };
  use(b);
  setTimeout(b.cb, 0);
  return <p />;
}
export function BundleWritten() {
  const b: { cb: () => void } = { cb: () => { useContext(Present); } };
  b.cb = () => {};
  setTimeout(b.cb, 0);
  return <p />;
}
export function BundleLet() {
  let b = { cb: () => { useContext(Present); } };
  setTimeout(b.cb, 0);
  b = { cb: () => {} };
  return <p />;
}
export function BundleComputed() {
  const b = { cb: () => { useContext(Present); } };
  const k = "cb" as const;
  setTimeout(b[k], 0);
  return <p />;
}
export function BundleSpread() {
  const b = { cb: () => { useContext(Present); } };
  const c = { ...b };
  setTimeout(c.cb, 0);
  return <p />;
}
export function AliasLet() {
  const f = () => { useContext(Present); };
  let a = f;
  setTimeout(a, 0);
  a = () => {};
  return <p />;
}
export function AliasChosen(props: { on?: boolean }) {
  const f = () => { useContext(Present); };
  const g = () => {};
  const a = props.on ? f : g;
  setTimeout(a, 0);
  return <p />;
}
export function AliasThenCaught() {
  const f = () => { useContext(Present); };
  const a = f;
  Promise.resolve().then(a).catch(() => {});
  return <p />;
}
export function MemoUnread() {
  const b = { cb: () => useContext(Missing) };
  createMemo(b.cb);
  return <p />;
}
export function BundleCaught() {
  const b = { cb: () => { useContext(Present); } };
  runWithOwner(null, () => {
    try {
      b.cb();
    } catch {}
  });
  return <p />;
}
