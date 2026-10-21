import { createContext, useContext } from "solid-js";

// Default-less and never provided: rendering `Missing` throws ContextNotFoundError.
const MissingContext = createContext<string>();
// Default-less and provided above `Reader`: no throw.
const ProvidedContext = createContext<string>();
// With a default: the read returns the default, no throw (ADR 0349).
const DefaultedContext = createContext<string>("fallback");

// Throws ContextNotFoundError when rendered: no provider exists for MissingContext.
export function Missing() {
  const value = useContext(MissingContext);
  return <p>{value}</p>;
}

// Control: a default returns the default.
export function Defaulted() {
  const value = useContext(DefaultedContext);
  return <p>{value}</p>;
}

// Control: the reader is rendered under its provider.
export function Provided() {
  return (
    <ProvidedContext value="provided">
      <Reader />
    </ProvidedContext>
  );
}

function Reader() {
  const value = useContext(ProvidedContext);
  return <p>{value}</p>;
}
