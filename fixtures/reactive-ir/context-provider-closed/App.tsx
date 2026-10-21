import { createContext, createMemo, useContext } from "solid-js";
import { readRemote } from "./remote";

type Theme = "light" | "dark";

// Default-less and never provided: a read under an owner throws ContextNotFoundError.
const ThemeContext = createContext<Theme>();
// Default-less and provided above `Reader`.
const ProvidedContext = createContext<Theme>();
// With a default: the read returns the default, so nothing throws.
const DefaultedContext = createContext<Theme>("light");
// Escapes: stored in an object literal, which any holder may use to provide it.
const EscapedContext = createContext<Theme>();
export const holders = { theme: EscapedContext };

// Positive: a component body reads a default-less context no provider supplies.
export function Panel() {
  const theme = useContext(ThemeContext);
  return <i>{theme}</i>;
}

// Positive: a computation a component creates runs under an owner.
export function Memo() {
  const label = createMemo(() => useContext(ThemeContext));
  return <i>{label()}</i>;
}

// Positive, across files: `readRemote` reads RemoteContext under its caller's owner.
export function Remote() {
  return <i>{readRemote()}</i>;
}

// Not this rule: an event handler has no owner, so the read throws NoOwnerError (SC4001).
export function HandlerRead() {
  return <button onClick={() => useContext(ThemeContext)}>read</button>;
}

// Negative: the reader is rendered under its provider.
export function Provided() {
  return (
    <ProvidedContext value="dark">
      <Reader />
    </ProvidedContext>
  );
}

function Reader() {
  const theme = useContext(ProvidedContext);
  return <i>{theme}</i>;
}

// Negative: a default makes the read a value, not a throw.
export function Defaulted() {
  const theme = useContext(DefaultedContext);
  return <i>{theme}</i>;
}

// Negative: an escaped context may be provided by the code that holds it.
export function Escaped() {
  const theme = useContext(EscapedContext);
  return <i>{theme}</i>;
}

// Shadowed: a provider for a different binding that shares the name. The census is
// keyed by binding, so the module's `ThemeContext` still has no provider.
export function Shadow() {
  const ThemeContext = createContext<Theme>();
  return (
    <ThemeContext value="light">
      <i>shadowed</i>
    </ThemeContext>
  );
}
