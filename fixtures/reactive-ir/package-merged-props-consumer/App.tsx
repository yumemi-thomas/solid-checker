import { withDefaults, withOverrides } from "reactive-package";

// The claim under test. `withDefaults`'s contract states a `merged-props`
// return reaching through to argument 0, and argument 0 here is the component's
// own props: the merged object carries their reactivity, so destructuring it
// snapshots values that will change.
export function Card(props: { label: string }) {
  const { label } = withDefaults(props);
  return <div>{label}</div>;
}

// The same export and the same claim, with a plain object at argument 0. A
// merge of plain objects is plain, so this destructure loses nothing. The arm
// answers `false` here rather than `unknown` -- which is what makes the claim
// worth stating at all.
export function Static() {
  const { label } = withDefaults({ label: "fallback" });
  return <div>{label}</div>;
}

// `withOverrides` reaches through to argument *1*. The props are there, so this
// is the positive case again at a different index -- an implementation that
// ignored the index would report `Static` too, and one that hard-coded argument
// 0 would miss this.
export function Overridden(props: { label: string }) {
  const { label } = withOverrides({ label: "fallback" }, props);
  return <div>{label}</div>;
}

// Props at argument 0, which `withOverrides` does not name. Clean, and clean is
// an **under-report** that ADR 0109 states explicitly: a real merge carries
// every source's reactivity, and the census certifies only that reads reach
// through to the named argument. It is pinned here so the incompleteness is
// visible in a snapshot rather than only in prose.
export function Defaulted(props: { label: string }) {
  const { label } = withOverrides(props, { label: "fallback" });
  return <div>{label}</div>;
}
