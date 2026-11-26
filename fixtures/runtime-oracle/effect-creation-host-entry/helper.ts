import { createMemo } from "solid-js";

export function fromHandler(): void {
  createMemo(() => 9);
}
