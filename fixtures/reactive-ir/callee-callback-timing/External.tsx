import { createSignal } from "solid-js";
import { subscribe } from "ext-lib";

// Uncertifiable: a package export with no body in the project and no accepted
// contract; nothing says when, or whether, it invokes the literal.
export function PackageCallee() {
  const [n] = createSignal(0);
  subscribe(() => console.log(n()));
  return <span>{n()}</span>;
}
