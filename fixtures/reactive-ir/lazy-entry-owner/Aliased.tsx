import { onCleanup } from "solid-js";

// Loaded through an alias of `lazy` (`import { lazy as load }`) and through a namespace (`S.lazy`): the facts name a `lazy` read only for the identifier `lazy`, so these
// loads escape with no name. Open (the cost of a facts-schema bump to widen it, docs/open-work.md).
export function ViaAlias() {
  onCleanup(() => {});
  return <p>alias</p>;
}
export function ViaNamespace() {
  onCleanup(() => {});
  return <p>namespace</p>;
}
