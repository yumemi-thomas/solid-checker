import { createContext, createMemo, useContext } from "solid-js";

// Default-less and never provided: a computation a component creates throws when it runs.
const MemoContext = createContext<string>();
// With a default: the same computation returns the default (control).
const DefaultedContext = createContext<string>("fallback");

// Throws ContextNotFoundError when rendered: the memo runs the read under its owner.
export function Memo() {
  const label = createMemo(() => useContext(MemoContext));
  return <p>{label()}</p>;
}

// Control: the defaulted read returns the default.
export function DefaultedMemo() {
  const label = createMemo(() => useContext(DefaultedContext));
  return <p>{label()}</p>;
}
