import { createMemo } from "solid-js";

// The other candidate for `./ambiguous`. It exports the same name with a
// different value, so guessing by file-enumeration order would produce a
// *proven* accessor claim sourced from the wrong module.
export const ambiguousTracked = createMemo(() => "/ambiguous-directory");
