import "solid-js";

// A local named `globalThis` is no global object: its `in` test and handing it on reach no global, before ADR 1077 and after.
export function probe(globalThis) {
  const deno = "Deno" in globalThis;
  return [deno, globalThis];
}
