import { action, createMemo } from "solid-js";

// An action another module holds: its body is not walked from the leaf.
export const imported = action(function* () {
  createMemo(() => 1);
});
