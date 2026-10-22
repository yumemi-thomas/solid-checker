import { createContext, useContext } from "solid-js";

// No package.json: the program is open, so unseen code may provide this context.
// The read makes no provider claim here.
const OpenContext = createContext<string>();

export function Open() {
  const value = useContext(OpenContext);
  return <i>{value}</i>;
}
