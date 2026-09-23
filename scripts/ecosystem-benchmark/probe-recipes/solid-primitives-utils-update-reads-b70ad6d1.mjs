// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`,
// on the published `./immutable` runtime case
// `artifact-case:b70ad6d1290a49f1c7da26503bd848d3979fbb44e3e0fca19c60ed92c59bae47`.
//
// Demand-scoped: 5 call sites across the pinned consumer corpus name this
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
// `update(...args)` is `withCopy(args[0], obj => …)`: it shallow-copies the
// caller's object or array, then either sets a key, calls the caller's setter
// with the previous value, or recurses one level deeper. Every read is of the
// caller's structure (ADR 0034); every write is on the copy the package made.
//
// What it does not do: close the domain. The census refuses this candidate on
// its own premises -- the transcript is open with `callSignatureNotUnique`, or
// the form states no reviewed subject root -- and a veto cannot supply a
// premise. This recipe is here because it makes that refusal *visible*: without
// it the candidate is withheld as `no recipe in corpus` and weakened out of the
// plan before its census ever runs. See `docs/precision-backlog.md`, 2026-09-18.
import { update } from "@solid-primitives/utils/immutable";

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const expect = (label, ok) => {
  if (!ok) throw new Error(`${label} answered unexpectedly`);
};

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const flat = { a: 1, b: 2 };
  expect("update sets a key", same(update(flat, "a", 9), { a: 9, b: 2 }));
  expect("update copies", update(flat, "a", 9) !== flat);
  expect("update leaves the caller's object alone", same(flat, { a: 1, b: 2 }));

  let setterCalls = 0;
  let previous;
  const answered = update(flat, "a", prev => {
    setterCalls += 1;
    previous = prev;
    return prev + 1;
  });
  expect("update ran the caller's setter once", setterCalls === 1);
  expect("update handed it the previous value", previous === 1);
  expect("update applied it", same(answered, { a: 2, b: 2 }));

  const nested = { foo: { bar: { baz: 123 } } };
  expect(
    "update reaches a nested key",
    same(update(nested, "foo", "bar", "baz", 124), { foo: { bar: { baz: 124 } } })
  );
  expect("update left the nested original alone", same(nested, { foo: { bar: { baz: 123 } } }));

  const list = [1, 2, 3];
  expect("update on an array copies the array", same(update(list, 0, 9), [9, 2, 3]));
  expect("update left the caller's array alone", same(list, [1, 2, 3]));
  // No emit is the point: nothing contradicted the closure.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
