import { createContext, useContext } from "solid-js";

const HookContext = createContext<number>();

export function useHookValue() {
  return useContext(HookContext);
}
