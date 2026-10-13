# Owned reads: two live observations and their limits

Date: 2026-09-12. Status: isolated experiment; no production policy, recipe,
contract, receipt, or finding changed.

**The current worker can carry a real veto for an owned tracked read.** Two
mechanisms work on the cached `@solidjs/signals@2.0.0-rc.3`: invalidating a
known source through an ordinary package operation, and inspecting the probe's
tracking sources through the published **development** API. Neither observes
all reads, and neither infers authorship from the observation alone.

The second result qualifies the older [reads observation design,
§ 3](2026-09-10-reads-veto-observation-design.md#3-why-it-is-the-wrong-observation):
reading minified graph fields is not necessary for a development artifact
whose exact runtime exports `DEV.getSources`. Production exports `DEV` as
`undefined`; changing it to development would change the artifact being
tested. This result does not authorize that substitution.

## What ran

Run the retained executable from any working directory:

~~~sh
node /Users/thomas/Documents/Github/solid-checker/docs/package-contract-v2/phase21/2026-09-12-reads-observation-experiment.mjs
~~~

[The executable](2026-09-12-reads-observation-experiment.mjs) launches all 16
scenarios through the repository's unmodified `runProbeSessions` audit driver
and `contract-probe-worker.mjs`. Each scenario uses a fresh worker process;
the test asserts that isolation and exact contradiction-marker counts. The
worker freezes its usual intrinsic prototypes and uses protocol
`solid-checker-runtime-probe-v7`. The script records package version, runtime
entry hashes, and subject hash in its JSON output.

Measured with Node `v24.11.1`: **16/16 completed and passed**, about 0.6 seconds
for the worker run. The package already existed under
`rust/target/tsc-oracle/v2/node_modules`; no install or network lookup ran.
Both profiles use the published cached package bytes, not a stub. Each subject
and its observer import the same runtime module instance.

This is the **audit path**, not a Rust certification transaction. It verifies
raw observations and marker reachability. It takes no private artifact copy,
establishes no certification authority, and produces no accepted contract.
The printed entry hashes are measurement identities, not a claim that the
whole dependency tree was authenticated.

## The subject and the observations

[The subject](2026-09-12-reads-observation-experiment.subject.mjs) creates a
private signal in its own factory and returns package-authored functions that
read it, ignore it, or invoke caller-authored code. The recipe supplies no
signal to the subject. The subject exposes a normal setter operation but no
read counter, observer, or graph-inspection method. It receives neither the
session nor the harness.

For the **production invalidation** experiment, the probe calls the subject
inside the compute phase of its own `createEffect`, invokes the package's
setter, and calls `flush`. A second compute is the observation. Counting
compute executions, instead of effect apply calls or changed returned values,
also detects a read whose value is discarded.

For the **development source-count** experiment, the probe uses
`DEV.getSources(getObserver()).length` immediately after the subject call in
a fresh compute scope. These are published exports declared in
`@solidjs/signals/dist/types/core/dev.d.ts` and `core/owner.d.ts`; the probe
does not inspect private properties. **This experiment invokes no setter.**

| Subject operation | Production compute count after package mutation | Development sources after initial call | Interpretation |
| --- | ---: | ---: | --- |
| Read private signal | 1 → 2 | 1 | Live contradiction to this subject's `reads: []` |
| Read private signal, discard result, return constant | 1 → 2 | 1 | Same contradiction despite unchanged return value |
| Return constant without reading | 1 → 1 | 0 | Negative control stays clean |
| Invoke caller accessor reading caller signal | 1 → 1 | 1 | Caller-owned read; source count alone would misattribute it |
| Read caller getter backed by caller signal | 1 → 1 | 1 | Same ownership boundary through a property |
| Invoke caller callback that reads the package source | 1 → 2 | 1 | Both raw observations collide with the owned-read case |
| Read private signal through `untrack` | 1 → 1 | 0 | Both mechanisms miss this real read |
| Read private signal through `untrack`, discard result | 1 → 1 | 0 | Both mechanisms miss it, and the return value does not reveal it |

The two caller-source controls really subscribe: after mutating the caller's
source their compute counts increase from 1 to 2. This confirms that the
production negative result follows the chosen source, not broken tracking.
The untracked reader returns 0 initially and 1 on an explicit call after the
production mutation, despite causing no rerun. The discarded untracked read
returns 0 both times.

Only the first two reviewed subject shapes emit `read-operation`, in each
profile. The known fixture bodies establish their authorship. The code's
`mayVeto` table is **not an ownership detector**: for the collision case the
raw observation is positive and the experiment deliberately declines to
classify it as this export's read. These checks would be misleading if
presented as a synthesized recipe that had solved attribution.

## What remains unsupported

- **Untracked reads.** The domain includes reads with `tracking: untracked`.
  Both experiments miss them, including the discarded-value example. The
  limitation is observed; it is not a proof that every possible observer
  must miss them.
- **Authorship.** Subscription identity answers which source was read, not
  whether the subject or caller-authored code performed the read. Fresh
  inert callback/object arguments can avoid the particular caller controls
  tested here, but are not a proof about all ambient third-party callbacks.
- **Unknown mutation controls.** Production invalidation requires a known
  operation that changes the observed source. The harness cannot derive one
  from an arbitrary function signature.
- **Artifact and runtime identity.** The development API applies only where
  the actual selected dependency artifact exposes it, and must be the same
  runtime instance the subject uses. The experiment imports profiles
  explicitly and claims no production/development equivalence.
- **Scheduled reads and unvisited branches.** These synchronous samples do
  not establish observation coverage for deferred work, pending async
  sources, alternative branches, nested observers, other reactive runtimes,
  or every possible input.
- **Independent proof.** A veto never supplies the implementation census.
  Passing these observations would not resolve a hidden census refusal.

The existing proxy fixture already contains a narrower live observation via
an exposed module-owned trap counter. Consequently the five vacuous
ecosystem recipes in [the previous audit, § 65](2026-09-10-reads-veto-observation-design.md#65-every-reads-recipe-ever-written-is-vacuous-as-a-veto-2026-09-11)
demonstrate a limitation of those recipes; they do not prove that an owned read
can never be observed.

## Next bounded step

Do not start a large reads recipe campaign from this experiment. First select
**one existing development artifact** whose exact dependency instance exposes
`DEV.getSources`, whose normal invocation can use inert caller inputs, and
whose independent census survives when a throwing scaffold exposes it. Run
its clean invocation and a separate deliberately false implementation through
the same observation; require a reachable contradiction for the latter.

That would test whether the mechanism applies to a real ecosystem candidate,
before reviewing a narrowly scoped synthesized observation. A production-only
candidate instead needs an audited public mutation control. Neither route
should claim coverage of untracked or deferred reads.

The screenshot's 8,531 withheld `reads` entries cannot size that opportunity.
Recipe removal precedes census evaluation, so most entries expose only the
first blocker. The retained [§ 64.4 measurement](2026-09-10-reads-veto-observation-design.md#644-the-measurement)
found 102 census refusals and 71 throwing-scaffold failures among 173 claims
for one package; the latter are an upper bound on serviceable work. This
experiment ran no new ecosystem census and makes no coverage forecast.

Tracking-only demand scoping remains rejected for the reasons in
[the demand-scoping decision, § 13](2026-09-10-sc9005-demand-scoping-design.md#13-step-1-of-the-layer-move-the-concept-and-why-it-retires-steps-24).
An observer's inability to see an untracked read is not grounds to remove the
consumer's obligation to know about it.

## Verification and scope

The 16-scenario Node experiment and a scoped `git diff --check` passed. No
Cargo command, coverage regeneration, ownership gate, ecosystem rerun, or
`make verify` ran in this independent lane; the integration owner controls
shared build and handoff checks. No snapshots, bundled contracts, receipts,
or public schema artifacts changed. Production `reads` synthesis remains
withheld where no reviewed recipe serves the claim.
