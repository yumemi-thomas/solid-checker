import { createRoot } from "solid-js";
export function useRoot(cb: () => void) {
  createRoot(() => cb());
}
