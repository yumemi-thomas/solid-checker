# after-await-carrier-runs

**Claim (ADR 0566).** `reactive-read-after-await` (SC1002) proves a read in a callback a standard-library carrier invokes inline only where the carrier provably runs it at least once (ADR 0563's `carrier_runs::carrier_provably_runs`); a receiver that may be empty makes the read uncertifiable, never silent.

Violations: `OneElementLiteral`, `OneElementNamed` (`[1].map(hf)`), `CallReceiver` (`hf.call(null)`, `Reflect.apply`).

Uncertifiable, never a violation (each a violation on cd3056fd4): `EmptyLiteral`, `EmptyNamed`, `UnknownLength` (a prop array), `SetForEach`. Already uncertifiable on cd3056fd4: `MappedAsyncEmpty` (`Promise.all` over a `.map`), `ShadowedForEach` (a project `forEach` is no carrier: ADR 0565's obligation).

Runtime: rc.17 warns `UNTRACKED_READ_AFTER_AWAIT` at the violations (the fuzzer's `handoff-foreach`, `-map`, `-call`, `-reflect` agree rows on 644bd1a81) and reads nothing on an empty receiver. Stub: `async-carrier-provably-runs`'s. `tsc --noEmit` clean against the stub and the published rc.17 typings.
