import { createContext, useContext } from "solid-js";

// Default-less, exported, and never provided anywhere in the program.
export const RemoteContext = createContext<string>();

export function readRemote(): string {
  return useContext(RemoteContext);
}
