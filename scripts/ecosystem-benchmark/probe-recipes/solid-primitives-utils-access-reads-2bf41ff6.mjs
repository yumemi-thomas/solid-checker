// Hand-authored `reads: []` veto for `@solid-primitives/utils@7.0.0-next.4`,
// on the published `.` runtime case
// `artifact-case:2bf41ff69a22f6022ae4b8485df1bd52096d111e906fc32792e33004f7956ee0`.
//
// `access` is `(v) => typeof v === "function" && !v.length ? v() : v`, and it
// is the export in this set that actually *executes caller code*. That makes
// it the one worth writing by hand, because it is where the domain boundary
// is, not where it is absent.
//
// The boundary: a read reached through a caller-supplied value is the
// caller's (ADR 0034), not this closure's. `access` owns no reactive-shaped
// source -- the artifact case carries no `runtime-accessor-installation`
// hazard, so the census states syntactically that `dist/index.js` installs no
// accessor -- and every read it can possibly cause is a read of something the
// caller handed it.
//
// So the last sample is the one that matters. It passes an accessor that
// reads a getter **this recipe owns**, asserts the getter actually ran, and
// then deliberately does not emit. That assertion is what stops this from
// being a vacuous recipe: the observation apparatus is proven live on every
// run, so a future version of `access` that started consulting something of
// its own would be sampled by a recipe known to be capable of seeing a read,
// rather than by one that never could.
//
// What it cannot do: establish the closure. Finite samples only falsify, and
// the authenticated implementation census remains the proof. It hands the
// package no `session` and no `harness`.
import { access } from "@solid-primitives/utils";

export async function runProbeSession(_session, harness) {
  const zeroArity = () => "called";
  const oneArity = _unused => "not called";
  const plain = { value: 1 };

  for (const [input, expected] of [
    [plain, plain],
    [zeroArity, "called"],
    [oneArity, oneArity],
    [undefined, undefined],
    [0, 0]
  ]) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const answered = access(input);
    if (!Object.is(answered, expected)) {
      throw new Error(`access answered ${String(answered)}`);
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }

  // The caller-owned read, and the proof that this recipe can see one.
  let observed = 0;
  const ownedByTheRecipe = {
    get current() {
      observed += 1;
      return "read";
    }
  };
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const answered = access(() => ownedByTheRecipe.current);
  if (answered !== "read" || observed !== 1) {
    throw new Error(
      `the caller-supplied accessor did not run: answered ${String(answered)}, observed ${observed}`
    );
  }
  // No emit. `observed` counts a read of a source the *caller* owns, which is
  // ADR 0034's, and emitting here would contradict a closure that is about
  // sources the package owns.
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
