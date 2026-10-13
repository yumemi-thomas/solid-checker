# Whole-package composition experiment: timer baseline and proposed boundary

Date: 2026-10-01. Status: baseline measured; new contract form awaiting the
owner's decision. No verifier, schema, producer, accepted-tier or fixture changes.

The target is `@solid-primitives/timer@1.4.5-next.1`, using the retained exact
published install and Solid 2.0.0-rc.9. Node already has four of five exports
clean. `createPolled` has callbacks, reads and creates closed; its return is the
last open domain. Completing all five node exports is a bounded whole-package
test. It would not complete the all-host checkpoint: browser has zero clean
timer exports and independent unresolved domains.

## What the source requires

The server branch is:

```js
if (isServer) {
  const v = fn(value);
  return () => v;
}
```

The returned function performs no new callback invocation. It retains the value
that the factory's callback already returned. That value can be a primitive,
object, function or reactive value. Existing `described-callable` returns admit
plain values, their own signal reads, and results of callbacks the returned
function invokes. None describes this capture. The existing refusal is correct;
using `plain` here would be unsound. This is an expressiveness limit in addition
to the missing proof composition.

## Published-package baseline

The offline runner independently generates, verifies and authenticates ordinary
proposals for node and browser. It checks the complete five-export surface and
four consumer specimens per host against TypeScript 5.9.3 and real declarations.
All eight type checks pass. Sixteen analyses compare the compiled tier with the
reissued receipt; their findings agree exactly.

| Host | Clean exports | Complete package | Whole-surface consumer |
| --- | ---: | --- | --- |
| node | 4/5 | no | SC9005, returns incomplete |
| browser | 0/5 | no | SC9005 and ownership findings |

On node the intentional snapshot specimen and a specimen capturing a signal
accessor also receive SC9005 only. The untracked factory-callback specimen
receives SC9005 and SC1001 violations. These record the current diagnostic
behavior; they do not establish that every server read should be diagnosed.
Server signal behavior must be respected when evaluating future findings.
Browser retains independent callback, read, return and owner gaps. Its
module-scope whole-surface specimen is an ownership negative control, not a
correct-use browser sample.

A runtime falsifier executes the exact published node artifact. The factory
callback returns a function token. It runs once, both subsequent reads return
the identical token, and the token remains callable. This disproves an always
plain return and distinguishes retaining a result from invoking again. Runtime
samples confer no proof authority.

Evidence is in `rust/target/package-composition-baseline/results.json`, with
checker/producer hashes, exact artifact identity, receipts, audits, raw findings
and published-type outputs retained alongside it. No new complete package has
been demonstrated by this baseline.

## Proposed isolated implementation

Owner approval is required by the earlier instruction: "Ask the owner before
any new claim form or value shape." The pending question asks permission for a
described-callable return that retains a factory callback's earlier result.
Its meaning must state the source argument, preserve exact value identity and
assert no additional callback execution during the returned function's call.
It must remain distinct from the current invocation-result form's meaning.

The bounded proof would compose an authenticated factory callback call, an
immutable local initialization from that exact call, and the returned closure's
exact reference to that binding. It must bind source bytes and symbol/frame
identities, not names. Existing callback and other-domain censuses remain
authoritative. Unknown, mutated, shadowed, optional, member, async or ambiguous
sources refuse; another possible completion must be accounted for separately.
A synthesized veto must check captured-result identity and unexpected callback
execution, rather than treating this new completion as unchecked.

Consumer interpretation is part of the experiment. A callback result may itself
be callable or reactive. The new form must not project that unknown value as
proven non-reactive. Exact consumer callback-result facts should compose where
available; unresolved values must remain explicitly uncertifiable. A package
surface that closes while its consumer loses a valid finding fails the test.

The success bar is all five node exports independently authenticated with every
consumer domain closed, a clean published-type whole-surface consumer, and
focused misuse/correct-use controls with justified findings. Browser's stronger
claims must remain withheld until their own behavior is proved. Schema, wire,
canonical hashing, validation, generator, verifier, mandatory veto and consumer
changes must travel together on an isolated branch. No tier promotion follows
from a prototype result.

## Checks

Both native certifications authenticated and selected the exact case. The eight
published-type checks, sixteen consumer analyses and runtime falsifier passed.
JavaScript syntax and whitespace checks passed. Rust and accepted artifacts are
unchanged; full release verification, coverage/ownership reruns, tier generation
and checkpoint/app sweeps are deferred. The new form has not been implemented
while the owner decision is pending.
