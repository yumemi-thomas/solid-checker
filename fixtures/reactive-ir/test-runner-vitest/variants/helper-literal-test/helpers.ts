import { flush } from "solid-js";

// Exported from its own module; calls its argument during the call.
export function fireAndFlush(fire: () => void): void {
  fire();
  flush();
}
