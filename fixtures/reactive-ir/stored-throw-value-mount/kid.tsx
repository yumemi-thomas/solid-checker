import { createContext, useContext } from "solid-js";

const Ctx = createContext<number>();

export function Handled() {
  return <i>{useContext(Ctx)}</i>;
}

export function Routed() {
  return <i>{useContext(Ctx)}</i>;
}

export function Reported() {
  return <i>{useContext(Ctx)}</i>;
}

// A component with no value use anywhere stays as it is.
export function Unused() {
  return <i>{useContext(Ctx)}</i>;
}
