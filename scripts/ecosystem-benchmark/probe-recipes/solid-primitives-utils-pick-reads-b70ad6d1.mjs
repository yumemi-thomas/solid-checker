// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`,
// on the published `./immutable` runtime case
// `artifact-case:b70ad6d1290a49f1c7da26503bd848d3979fbb44e3e0fca19c60ed92c59bae47`.
//
// Demand-scoped: 12 call sites across the pinned consumer corpus name this
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
// `pick(object, ...keys)` reduces the caller's key list onto a fresh `{}`,
// testing `k in object` and copying `object[k]`. Both reads are of the
// caller's object (ADR 0034), and it never touches a key the caller did not
// name -- the untouched getter below is what asserts that.
//
// What it does not do: close the domain. The census refuses this candidate on
// its own premises -- the transcript is open with `callSignatureNotUnique`, or
// the form states no reviewed subject root -- and a veto cannot supply a
// premise. This recipe is here because it makes that refusal *visible*: without
// it the candidate is withheld as `no recipe in corpus` and weakened out of the
// plan before its census ever runs. See `docs/precision-backlog.md`, 2026-09-18.
import { pick } from "@solid-primitives/utils/immutable";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const source = { a: "foo", b: "bar", c: "baz" };
  expect("pick names two keys", same(pick(source, "a", "b"), { a: "foo", b: "bar" }));
  expect("pick leaves the caller's object alone", same(source, { a: "foo", b: "bar", c: "baz" }));
  expect("pick with no keys", same(pick(source), {}));
  expect("pick skips an absent key", same(pick(source, "a", "nope"), { a: "foo" }));

  let picked = 0;
  let untouched = 0;
  const ownedByTheRecipe = {
    get taken() {
      picked += 1;
      return "read";
    },
    get left() {
      untouched += 1;
      return "unread";
    }
  };
  expect("pick over caller getters", same(pick(ownedByTheRecipe, "taken"), { taken: "read" }));
  // The apparatus is live: the named getter fired, and it is the caller's read
  // under ADR 0034, so nothing is emitted for it.
  expect("pick read the named key once", picked === 1);
  expect("pick never read the key it was not given", untouched === 0);
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
