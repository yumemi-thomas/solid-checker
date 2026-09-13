# Returns synthesis opportunity and first certification

The 1,146 `returns` closures withheld for `no recipe in corpus` are **78 distinct
semantic claim IDs**, repeated through 31 root probes. There are **24 attributed
package/version/export identities, plus two unattributed claims/rows**. They
are not 1,146 independent helpers. Exact retained proposals
identify **39 claims / 1,076 rows** as whole-parameter identity returns. That
establishes a substantial shape opportunity, but does not establish how many
will pass signature acquisition, the implementation census, or the veto.

This inventory used the screenshot's baseline,
`rust/target/ecosystem-regression/report-0094.json`, whose 418 probes ran from
2026-09-12 04:13:45.079 UTC to 04:21:48.819 UTC. Its SHA-256 is
`7b673664a95292e40b206f39bdb820cd7df80e2f690c533db62afb2aeb76de5c`.
The report names `rust/target/release/solid-checker-rust`, `bin/solid-typefacts`,
and `scripts/ecosystem-benchmark/probe-recipes`. It does not retain executable
content digests; this is a historical report measurement, not a fresh result
from the current dirty worktree. The earlier `report-t1200.json` has the same
1,146 rows and 78 claim IDs in its missing-recipe returns ledger.

## Exact shape recovery

The report does not contain withheld operation payloads. The inventory joined
its exact `semanticClaimId` to 1,891 retained proposal sidecars, then selected
the companion contract summary using the candidate's entrypoint, runtime path,
runtime digest, artifact closure digest, declarations path, and declarations
digest. The artifact case and export must also match. The companion contract's
package name/version must agree with the report's node identity wherever the
report provides one. Conflicting matches fail the inventory rather than
silently choosing one.

The baseline has eight rows without a node identity, all in the
`reused-proposal` lane. Six match exact retained contracts for
`@solid-primitives/i18n@3.0.0-next.4`; the contract supplies their attribution.
Two have no recovered contract and remain unattributed. Their root probe is
`@solid-primitives/i18n@2.2.1|solid1|only`, but a root probe label is not substituted
for the missing package identity. Of all rows, 1,138 are attributed by report
node, six by an exact retained contract, and two remain unattributed.

| Classification | Closure rows | Distinct claim IDs | Attributed package/version/export identities |
| --- | ---: | ---: | ---: |
| Recovered whole-parameter identity | 1,076 | 39 | 12 |
| Unrecovered shape | 70 | 39 | 15, plus two unattributed rows |
| Total | 1,146 | 78 | 24, plus two unattributed rows |

The identity columns overlap: three exports have both recovered and unrecovered
artifact cases. No recovered candidate was empty or a different return shape.
**Unknown means the exact proposal was not retained in the searched locations**;
it does not mean unsupported behavior. The script does not infer from an export
name, adjacent artifact, or published function body.

| Package/version | Export | Rows | Claim IDs | Rows with recovered identity shape |
| --- | --- | ---: | ---: | ---: |
| `solid-js@1.9.14` | `Match` | 437 | 8 | 437 |
| `solid-js@1.9.14` | `onCleanup` | 437 | 8 | 437 |
| `solid-js@1.9.14` | `dynamicProperty` | 166 | 6 | 166 |
| `motion-utils@12.39.0` | `noop` | 26 | 1 | 0 |
| `@solidjs/web@2.0.0-rc.3` | `withMeta` | 14 | 11 | 4 |
| `@solidjs/web@2.0.0-rc.3` | `registerServerFunction` | 8 | 8 | 0 |
| `@tanstack/query-core@5.101.0` | `keepPreviousData` | 8 | 1 | 8 |
| `@solidjs/web@2.0.0-rc.3` | `asyncArg` | 5 | 5 | 0 |
| `@solid-primitives/i18n@3.0.0-next.4` | `identityResolveTemplate` | 4 | 2 | 4 |
| `@solid-primitives/i18n@3.0.0-next.4` | `missingKeyAsPath` | 4 | 2 | 4 |
| `@solid-primitives/i18n@3.0.0-next.4` | `template` | 4 | 2 | 4 |
| `@tanstack/query-core@5.101.4` | `keepPreviousData` | 4 | 1 | 4 |
| `@tanstack/query-core@5.102.5` | `keepPreviousData` | 4 | 1 | 0 |
| `solid-js@1.9.14` | `createDeferred` | 4 | 4 | 4 |
| `solid-js@1.9.14` | `createMutable` | 4 | 4 | 2 |
| `solid-js@1.9.14` | `unwrap` | 4 | 4 | 2 |
| `@solidjs/web@2.0.0-rc.0` | `withMeta` | 3 | 1 | 0 |
| `@tanstack/query-core@5.102.8` | `keepPreviousData` | 2 | 1 | 0 |
| Unattributed | `identityResolveTemplate` | 1 | 1 | 0 |
| Unattributed | `template` | 1 | 1 | 0 |
| `@tanstack/form-core@2.0.0-alpha.2` | `createErrorVisibility` | 1 | 1 | 0 |
| `@tanstack/form-core@2.0.0-alpha.2` | `formOptions` | 1 | 1 | 0 |
| `@tanstack/virtual-core@3.17.8` | `defaultKeyExtractor` | 1 | 1 | 0 |
| `seroval@1.6.4` | `createPlugin` | 1 | 1 | 0 |
| `seroval@1.6.4` | `createReference` | 1 | 1 | 0 |
| `solid-js@2.0.0-rc.3` | `$$component` | 1 | 1 | 0 |

The first three exports account for 1,040 rows, or 90.8% of the recipe-blocked
returns ledger. Closing them could make the aggregate percentage jump while
adding only three distinct package/version/export behaviors. Report both units
when measuring the implementation.

## Consumer impact and next measurement

The retained [Seroval experiment](2026-09-10-real-package-returns-recipe.md)
already measures a consumer that binds and reads `createReference`'s result:
SC9005 loses `returns` and retains `reactiveReads`. It used `seroval@1.5.6`, a
hand recipe, and a synthetic certification importer. That is evidence of the
consumer benefit of closing this domain; it is not a measurement of synthesized
vetoes or of the inventory's `seroval@1.6.4`.

For a fresh implementation comparison, `@solid-primitives/i18n@3.0.0-next.4`
offers three recovered identity exports across two exact claims apiece.
`solid-js@1.9.14` offers the larger ledger impact. Run control and treatment
against the same package artifacts, recipe corpus, and selected conditions,
then count newly closed claim IDs and remaining refusal reasons. A missing
recipe is the first blocker: the inventory has not run the subsequent census.
Do not convert the 1,076 shape-matched rows into a promised certification gain.

## First fresh certification

The integration owner ran the fresh debug checker through `certifyContract` on
the retained `@solid-primitives/i18n@3.0.0-next.4` install. The acquisition used
the exact SHA-512-verified published archive in the registry cache; its supplied
`fetch_` throws on every cache miss. No network or reinstall was needed.

The run exited successfully and published a local accepted catalog. Its audit
reports `receiptAuthenticated: true` and `exactCaseSelected: true`. One artifact
case closes `returns` for **`identityResolveTemplate`, `missingKeyAsPath`, and
`template`**, each returning whole parameter 0. The ledger has seven exports
with at least one closed domain, comprising seven `creates` closures and three
`returns` closures. The audit's `count: 7` counts exports, not domain closures.

Three `creates` claims stay withheld: `prefix` on coercion of a written
parameter, `resolveTemplate` on an unknown accessor rooted at a written
parameter, and `scopedTranslator` on template coercion of a nested parameter.
The run does not establish a `reads` closure or a clean consumer result.

The [compact measurement](2026-09-12-returns-synthesis-i18n-measurement.json)
retains public artifact identities/digests, relevant receipt roots, return
operations, and audit acceptance fields. The main document and receipt bytes
were rehashed and checked against their catalog digests when extracting it.
It is diagnostic evidence, not a replayable acceptance receipt. The original
output is `/private/tmp/returns-synthesis-i18n-yw021b`.

**This is an actual observation of three closed returns domains, not a measured
before/after delta.** No contemporaneous control binary run was made, and the
historical inventory baseline has different executable provenance. It confirms
the mechanism works on a real package without establishing corpus-wide yield.

The [configurable offline runner](2026-09-12-returns-synthesis-offline.mjs)
preserves that acquisition path. It accepts an existing installed package root
and exact SRI, optional conditions, registry cache, and output parent; every run
creates a separate local issuer/catalog directory. Its private issuer file
stays in that temporary directory and is not included in the measurement.

```sh
SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/debug/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
node docs/package-contract-v2/phase21/2026-09-12-returns-synthesis-offline.mjs \
  --package-root /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-iXqdsi/node_modules/@solid-primitives/i18n \
  --integrity 'sha512-r5ZwXcxsE0NEC4MCs5nphs9gnEdDkvZ5zp3P/I0h0wHHXBQi1fuXkQv857HBgTi2Mwja7vS5D6nePyQQxd9q/A=='
```

This package root includes its original lockfile/dependency installation. If
that retained tree or an authenticated cache entry is unavailable, report the
blocker; do not substitute package versions or allow the runner network access.

## Inventory reproduction and verification

The companion [inventory script](2026-09-12-returns-synthesis-inventory.py) reads
existing JSON only, emits a compact per-export inventory and per-claim source
paths/digests, and never invokes an analyzer, installs packages, or changes
contracts. The explicit temporary root below is the one used for this run;
provide other retained proposal globs when reproducing elsewhere.

```sh
python3 docs/package-contract-v2/phase21/2026-09-12-returns-synthesis-inventory.py \
  rust/target/ecosystem-regression/report-0094.json \
  --proposal-glob '/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-certify-*/*/*.proposal.json' \
  --proposal-glob '/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-certify-*/*.proposal.json' \
  --proposal-glob '/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-out-*/*.proposal.json'
```

Only root `results` are counted; family summaries repeat those rows. Dependency
details are attributed to `node.package` and `node.version`, or to an exact
retained contract when the node is missing. The root probe's package is never
substituted. A reproduction with missing temporary files keeps the same
ledger counts and reports more unknown shapes.

Validation: the script completed against the 418-probe baseline, with every
recovered claim subject, artifact, and package identity checked for consistency.
An independent read of `report-t1200.json` confirmed the same row/claim
denominators. Four bounded attribution controls checked a matching report node,
derivation from an exact contract when the node is missing, withholding
attribution when both are absent, and rejection when the node package conflicts
with the contract. The configurable runner passed `node --check`; its original
fixed-input form executed the measured certification. No fixtures, snapshots,
bundled contracts, or schemas changed in
this inventory work. The fresh certification created temporary accepted
artifacts and the compact checked-in measurement described above. No Cargo,
ecosystem rerun, or expensive repository gate ran in this inventory lane;
implementation validation belongs to the integration owner.
