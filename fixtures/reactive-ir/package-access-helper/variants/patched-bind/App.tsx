import "./patch";
import { createSignal } from "solid-js";
import { access } from "@solid-primitives/utils";

// Uncertifiable: the project replaces `Function.prototype.bind` before any
// getter is created, and `read.bind(null, node)` builds every rc.13 getter, so
// `length` is not proven 0 here.
export function UncertifiablePatchedBind() {
  const [value] = createSignal(1);
  const initial = access(value);
  return <span>{initial}</span>;
}
