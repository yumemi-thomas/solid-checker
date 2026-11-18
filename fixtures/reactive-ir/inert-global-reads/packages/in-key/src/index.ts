import "solid-js";

// A key the syntax does not fix may run code to become one, and hands the object to `in`: the object escapes, any global may be written.
export const has = (key: string): boolean => key in globalThis;
