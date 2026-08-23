import { openAmbiguous, openSelected } from "conditional-variant-package";

// The two exports below carry the same two overlapping branches -- `browser`
// and `development`, both satisfied by this project's selected runtime
// (.solid-checker/runtime.json) -- and differ only in `precedence`. That is
// the whole claim under test: nothing else in the contract can decide which
// branch a runtime resolves.
//
// `openSelected` records the export map's own first-match-wins order, so the
// `development` branch (precedence 0) is the one the runtime reaches, and only
// that branch returns an accessor. Two named branches with no order recorded
// would be undetermined, so this resolution is precedence's alone.
const selected = openSelected();

export function TrackedRead() {
  return <div>{selected()}</div>;
}

export function UntrackedRead() {
  // Proven reactive read outside tracking, and provable only because the
  // ordered variants resolved. Had selection landed on the `browser` branch,
  // this accessor would carry no reactivity claim and the fixture would be
  // silent here; had it failed closed, the finding would be an uncertifiable
  // result at the import binding instead.
  const value = selected();
  return <div>{value}</div>;
}

// The fail-closed half. `openAmbiguous` has two named branches that both match
// this environment and declare the same `precedence`, so nothing in the
// contract says which one the resolver reaches first. Substituting either is a
// guess, so the import binding stays uncertifiable and the accessor read below
// is never reported as reactive.
const ambiguous = openAmbiguous();

export function AmbiguousRead() {
  const value = ambiguous();
  return <div>{value}</div>;
}
