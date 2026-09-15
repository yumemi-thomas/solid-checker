import { mergeProps } from "solid-js";

// The negative half of `component-props-merge-alias.tsx`, and the condition
// that makes the merge row honest: `mergeProps` carries its arguments'
// reactivity, it does not create any. Merging two object literals returns
// plain values -- 1.9.14's runtime only builds a `$PROXY` when a source is
// itself a proxy or a function, and otherwise copies each source's own
// descriptors -- so reading one back is not a stale read and must stay clean.
function Card(_props: { title?: string }) {
  const merged = mergeProps({ title: "Untitled" }, { subtitle: "None" });
  const title = merged.title;
  return <h1>{title}</h1>;
}

export { Card };
