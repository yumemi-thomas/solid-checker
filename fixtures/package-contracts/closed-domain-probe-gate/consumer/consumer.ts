// The consumer that keeps this fixture's `tsc` claim verified rather than
// asserted.
//
// Every export the fixture probes is used here, exactly as a real consumer
// would use it, and `tsc --noEmit` under `strict` with
// `moduleResolution: nodenext` reports nothing. That is what makes the closed
// claim this fixture certifies a claim the type system cannot make:
//
//   * `entry` and `driftedEntry` are both `(() => void) | undefined` here,
//     because that is what the declarations say. The runtime disagreement —
//     `driftedEntry` ships a number — is invisible to TypeScript, which is why
//     the probe veto has something to catch.
//   * `run` and `runCreatingOwner` are indistinguishable here, which is why a
//     `creates: []` claim about either is not TypeScript's to refuse and not
//     this checker's to certify from a declaration census; the implementation
//     census decides it, from `index.js`.
//   * `runAfterSettle` (from the sibling `primitive-consumer/` package) takes
//     the callback `onSettled` takes; the stub's signature is byte-faithful to
//     the audited one, so this line compiles exactly as it would against the
//     real `solid-js@2.0.0-rc.3`.
//
// Run it with:
//   packages/cli/node_modules/.bin/tsc --noEmit --project \
//     fixtures/package-contracts/closed-domain-probe-gate/consumer/tsconfig.json
import { driftedEntry, entry, run, runCreatingOwner } from "closed-domain-probe-gate-package";
import { runAfterSettle } from "closed-domain-probe-gate-primitive-consumer";

type Entry = (() => void) | undefined;

const declared: Entry[] = [entry, driftedEntry];

let entered = 0;
run(() => {
  entered += 1;
});
runCreatingOwner(() => {
  entered += 1;
});
runAfterSettle(() => {
  entered += 1;
});
for (const candidate of declared) {
  if (typeof candidate === "function") candidate();
}

export const observed: number = declared.length + entered;
