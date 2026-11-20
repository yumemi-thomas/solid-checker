import { createContext, useContext } from "solid-js";

// Provider-free (`createContext<T>()`, no provider in the program), read by a function App.tsx hands to an effect and also carries on in a handler (ADR 1142).
const HookContext = createContext<number>();

export function useHookValue() {
  return useContext(HookContext);
}
