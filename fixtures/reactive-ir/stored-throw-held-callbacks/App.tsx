import { createContext, runWithOwner, useContext } from "solid-js";
import { render } from "@solidjs/web";

// ADR 1044: a function held in a `const` copy, or in a property of a closed object literal (ADR 0351), runs where a use of that
// holder is called or handed off: its stored throw ends where that use's does.
const Present = createContext(7); // a default: a read with no owner is NO_OWNER_CONTEXT (SC4001)

// Violations (SC4001: a context read with no owner, NO_OWNER_CONTEXT).
export function BundleNullOwner() {
  const b = { cb: () => { useContext(Present); } };
  runWithOwner(null, b.cb);
  return <p />;
}
export function BundleTimer() {
  const b = { cb: () => { useContext(Present); } };
  setTimeout(b.cb, 0);
  return <p />;
}
export function BundleMethod() {
  const b = { cb() { useContext(Present); } };
  setTimeout(b.cb, 0);
  return <p />;
}
export function MemberCall() {
  return (
    <button
      onClick={() => {
        {
          const hf = () => { useContext(Present); };
          const ho = { m: hf };
          ho.m();
        }
      }}
    >
      go
    </button>
  );
}
export function AliasTimer() {
  const f = () => { useContext(Present); };
  const a = f;
  setTimeout(a, 0);
  return <p />;
}
export function AliasForEach() {
  const f = () => { useContext(Present); };
  const a = f;
  setTimeout(() => [1].forEach(a), 0);
  return <p />;
}
export function AliasOfAlias() {
  const f = () => { useContext(Present); };
  const a = f;
  const a2 = a;
  setTimeout(a2, 0);
  return <p />;
}
// Stays an obligation: the literal is written in the component body, where the owner net takes a function held in an object slot
// for one run anywhere (`function-run-unplaced`), whatever the copy does with it.
export function AliasOfMember() {
  const b = { cb: () => { useContext(Present); } };
  const a = b.cb;
  queueMicrotask(a);
  return <p />;
}

render(
  () => (
    <div>
      <BundleNullOwner />
      <BundleTimer />
      <BundleMethod />
      <MemberCall />
      <AliasTimer />
      <AliasForEach />
      <AliasOfAlias />
      <AliasOfMember />
    </div>
  ),
  document.body,
);
