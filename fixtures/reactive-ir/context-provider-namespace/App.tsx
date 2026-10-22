import { createContext, useContext } from "solid-js";
import * as contexts from "./contexts";

// Default-less, never exported, never provided: no namespace import can reach it,
// so the read is provider-free (SC4002).
const LocalContext = createContext<string>();

export function Local() {
  const value = useContext(LocalContext);
  return <i>{value}</i>;
}

// `NsContext` is exported by `./contexts` and reached here through a namespace
// import, which can name any export, so its provider census is not closed: no claim.
export function Namespaced() {
  const value = useContext(contexts.NsContext);
  return <i>{value}</i>;
}
