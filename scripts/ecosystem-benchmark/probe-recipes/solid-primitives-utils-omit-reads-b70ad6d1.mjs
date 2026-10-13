// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`,
// on the published `./immutable` runtime case
// `artifact-case:b70ad6d1290a49f1c7da26503bd848d3979fbb44e3e0fca19c60ed92c59bae47`.
//
// Demand-scoped: 3 call sites across the pinned consumer corpus name this
// export.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/immutable/index.js` installs no accessor and there
// is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// The hazard does the work an observation counter used to. What is left is
// what a mandatory veto is for: sample the export and emit only on
// contradiction.
//
// It hands the package no `session` and no `harness`.
//
// `omit(object, ...keys)` is `withObjectCopy(object, copy => keys.forEach(key
// => delete copy[key]))`. The copy is `Object.assign({}, object)`, so every
// property read is of the caller's object (ADR 0034) and every delete is on
// the object the package just made.
//
// What it does not do: close the domain. The census refuses this candidate on
// its own premises -- the transcript is open with `callSignatureNotUnique`, or
// the form states no reviewed subject root -- and a veto cannot supply a
// premise. This recipe is here because it makes that refusal *visible*: without
// it the candidate is withheld as `no recipe in corpus` and weakened out of the
// plan before its census ever runs. See `docs/precision-backlog.md`, 2026-09-18.
import { omit } from "@solid-primitives/utils/immutable";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const source = { a: "foo", b: "bar", c: "baz" };
  const answered = omit(source, "a", "b");
  expect("omit drops two keys", same(answered, { c: "baz" }));
  expect("omit copies", answered !== source);
  expect("omit leaves the caller's object alone", same(source, { a: "foo", b: "bar", c: "baz" }));
  expect("omit with no keys copies whole", same(omit(source), source));
  expect("omit tolerates an absent key", same(omit(source, "nope"), source));

  let copied = 0;
  const ownedByTheRecipe = {
    get kept() {
      copied += 1;
      return "read";
    }
  };
  expect("omit over a caller getter", same(omit(ownedByTheRecipe, "gone"), { kept: "read" }));
  // The apparatus is live: `Object.assign` read the getter once, and that read
  // is the caller's under ADR 0034.
  expect("the caller's getter fired exactly once", copied === 1);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
