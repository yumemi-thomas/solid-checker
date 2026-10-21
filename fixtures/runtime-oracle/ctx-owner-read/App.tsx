import { createContext, createMemo, useContext } from "solid-js";

type Theme = "light" | "dark";
// Defaulted, so the only throw in this case is the owner's: `getContext` checks
// the owner before the default, so a read with no owner throws NoOwnerError
// whatever the default (ADR 0349).
const ThemeContext = createContext<Theme>("light");

// Throws NoOwnerError when clicked: an event handler has no owner.
export function Clicked() {
  return <button onClick={() => useContext(ThemeContext)}>click</button>;
}

// Throws NoOwnerError when the timer fires: a timer callback has no owner.
export function Timed() {
  setTimeout(() => useContext(ThemeContext), 0);
  return <i>timed</i>;
}

// Control: a component body has an owner, so the read returns the default.
export function Body() {
  const theme = useContext(ThemeContext);
  return <i>{theme}</i>;
}

// Control: a computation a component creates has an owner.
export function Memo() {
  const label = createMemo(() => useContext(ThemeContext));
  return <i>{label()}</i>;
}
