import { createMemo } from "solid-js";

// One of two project files a bare `./ambiguous` specifier could name. The
// checker's relative resolver does not model a bundler's extension and
// directory-index precedence, so it refuses to pick between them.
export const ambiguousTracked = createMemo(() => "/ambiguous-file");
