import { flush } from "solid-js";
import { vi } from "vitest";

// The installer is named by a destructured key, not a member access: `const { useFakeTimers } = vi` resolves the key to the same declaration, so the program installs fake timers
// and the `setTimeout` callback's flush keeps the uncertifiable row of each code (three rows).
const { useFakeTimers } = vi;
useFakeTimers();

setTimeout(() => {
  flush();
}, 10);
