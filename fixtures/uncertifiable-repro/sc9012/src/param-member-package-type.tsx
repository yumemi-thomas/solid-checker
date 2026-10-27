// probus-hk ExploreMap.tsx:519 (maplibre-gl's `Map.unproject`): an effect passes an
// object typed by a package to a project helper that calls that type's method on
// it (here solid-js's `ChildrenReturn.toArray`).
import { children, createEffect, type ChildrenReturn } from "solid-js";
import type { JSX } from "@solidjs/web";

function countItems(resolved: ChildrenReturn): number {
  return resolved.toArray().length;
}

export function List(props: { children: JSX.Element; onCount: (count: number) => void }) {
  const resolved = children(() => props.children);
  createEffect(
    () => props.onCount,
    (onCount) => onCount(countItems(resolved)),
  );
  return <ul>{resolved()}</ul>;
}
