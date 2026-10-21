import { flush } from "solid-js";

export function commit(): void {
  flush();
}
