import { useForwardingProxy } from "./hook";

// A forward to a function in another file is not followed (ADR 0287).
export function useCrossFileForward() {
  return useForwardingProxy();
}
