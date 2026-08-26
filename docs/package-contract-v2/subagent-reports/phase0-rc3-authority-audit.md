# Sub-agent report: Phase 0 Solid RC.3 authority audit

**Agent:** `phase0_rc3_audit`

**Status:** Completed published-artifact and repository-authority audit

**Date:** 2026-08-27

**Repository changes during the audit:** None

This is the sub-agent authority audit that led to the replayable Phase 0 RC.3
audit artifacts. It identifies which conclusions can be reproduced directly
from the published packages, corrects the source-provenance record, and keeps
published RC.3 evidence distinct from the checker's still-RC.0 semantic
authority.

## Verdict

The published core Solid 2 RC.3 tuple is locally recoverable and independently
verifies:

- `solid-js@2.0.0-rc.3`;
- `@solidjs/signals@2.0.0-rc.3`;
- `@solidjs/web@2.0.0-rc.3`.

The checker itself is not yet RC.3-authoritative. Its bundled contracts,
runtime lock, registry-integrity memo, TypeScript oracle, dialect review
contracts, generated export indexes, example installation, fixtures, and many
semantic source comments still bind RC.0. The RC.3 ecosystem benchmark and
human review must therefore not be presented as proof that the current checker
certifies RC.3 behavior.

The existing RC.3 evidence review also contained one material factual error:
all three packages publish a usable registry `gitHead`.

## Exact published package identity

| Package | Registry SRI SHA-512 | Tarball SHA-1 | Tarball SHA-256 | Files | Unpacked bytes |
| --- | --- | --- | --- | ---: | ---: |
| `solid-js@2.0.0-rc.3` | `sha512-pmW6bRoTvfp/rN4jN7JmLvSaoIpFt7wm0Hi3j508S/smuJqUbRg3dQEjOPTkAwHW+McYnXrMG7cJ4AMNpLevtQ==` | `6ac639ca2558e283d941230e65fdaabffd745149` | `ad0427073f0c0467ae16b3396e53fc397f0cbf2e705de8534ffbe3bd0688d85a` | 43 | 555,790 |
| `@solidjs/signals@2.0.0-rc.3` | `sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==` | `6cb69036329c998e3e02bca2f30a26f1014a7999` | `9951cdd4943e5bbfce86b856c2738c63a452f17bd70bfb147f0f1998053b7075` | 107 | 1,446,682 |
| `@solidjs/web@2.0.0-rc.3` | `sha512-5ckKgOjem1pN5ADycOk6TjHmTtjbbN2fukqxo6RW3Oe3H7z0gaXWAdt8dLISto5/O4Nn8VxprFXFWpfy31+DUg==` | `4ac37e806a3948478955ff753beddec7b3def831` | `eb93aa325ebff6d054f227772b62b76be659b31d7fc19b87d19c7f3fbc26ab22` | 103 | 1,958,580 |

Fresh registry version metadata and independently hashed local tarballs agreed
on every value. Every archive member was a regular file; the archives contained
no symlinks. All non-wildcard runtime and declaration paths named by their
export maps existed in the corresponding tarball.

The audit found temporary copies at:

- `/private/tmp/solid-js-2.0.0-rc.3.tgz`;
- `/private/tmp/solidjs-signals-2.0.0-rc.3.tgz`;
- `/private/tmp/solidjs-web-2.0.0-rc.3.tgz`.

Those paths are ephemeral and are not baseline evidence by themselves.

## Source-provenance correction

Fresh registry metadata gives the same `gitHead` for all three packages:

`af6fee86e6dcfbf41869da2c607c82b1fd0939ce`

The local Solid repository established that the commit:

- exists in the official repository history;
- is contained by `upstream/next`;
- carries `solid-js@2.0.0-rc.3`, `@solidjs/signals@2.0.0-rc.3`, and
  `@solidjs/web@2.0.0-rc.3` tags;
- also carries the related RC.3 compiler and renderer package tags.

The three published `package.json` files are byte-identical to the corresponding
files at that commit. This contradicts the earlier statement in
`solid-rc3-evidence-review.md` that the packages did not publish a usable
`gitHead`.

The correction does not make source checkout bytes sufficient runtime
authority. Published `dist` and declaration outputs are generated and are not
tracked at that source commit. Their final identity remains the registry
tarball plus SRI and file hashes. The source commit establishes provenance and
the package manifests; it does not replace published-artifact verification.

## Manifest and export-map results

### `solid-js`

- Exports four subpaths: `.`, `./refresh`, `./types/*`, and `./package.json`.
- Depends on `@solidjs/signals@^2.0.0-rc.3`, `csstype@^3.1.0`,
  `seroval@~1.5.4`, and `seroval-plugins@~1.5.4`.
- Its root map distinguishes worker, browser development, browser production,
  Deno, Node, generic development, import, and require resolution.
- `development` does not by itself determine the browser artifact because
  ordered conditional-export traversal controls selection.

### `@solidjs/signals`

- Exports `.` and `./package.json`.
- Declares no runtime dependencies.
- Its import branch includes custom `test` and `development` conditions.
- Its require branch selects `dist/node.cjs` independently of those
  import-side branches.

### `@solidjs/web`

- Exports fourteen subpaths.
- Depends on `seroval@~1.5.4` and `seroval-plugins@~1.5.4`.
- Peers on `solid-js@^2.0.0-rc.3`.
- `./server-functions` and `./frames` have materially different nested
  condition structures.
- Runtime and declaration targets must be resolved and recorded separately,
  even where they appear under the same condition branch.

Canonical export-map projection hashes, computed with
`jq -cS '.exports' | shasum -a 256`, were:

| Package | SHA-256 |
| --- | --- |
| `solid-js` | `6bbdda21ba960ae57083a72677916ec99e9cec6382706d36ace135af5f895767` |
| `@solidjs/signals` | `5477b9c83b6458ff2331a658278da3af796dcd29806e200a77d4680fadecef47` |
| `@solidjs/web` | `9afa44c4c4100668e5baf8d56af85f6241d3fdf662f2d5a158895a3ff44f8260` |

Original JSON object order remains semantically relevant for resolution. These
sorted projection hashes are identity aids, not a replacement for preserving
the raw manifest bytes and ordered resolver trace.

## Runtime and declaration identity

Every representative runtime and declaration SHA-256 recorded in
`solid-rc3-evidence-review.md` was independently recomputed and matched.

Complete file-manifest digests were then computed over the lexically sorted
lines `SHA-256  relative-path`:

| Package | Complete file-manifest digest |
| --- | --- |
| `solid-js` | `9b4d99abaebb2fd3cc223df2d0d2e770c28bd12d36729faf9f7ce265a9873191` |
| `@solidjs/signals` | `9a83a76daf1548006e1b792050298521c82d45f2a5fbebef5884ebab453c0070` |
| `@solidjs/web` | `488cbb6fae6fad716a6724ced635a67db5f8e61d9479b3e79767787a2d45d92a` |

Before the replayable Phase 0 work, these complete manifests and their
derivation were not durably preserved in the repository.

## Exact closure gap

Published package ranges do not identify one exact installed execution or
declaration closure:

- `solid-js` imports and reexports the resolved `@solidjs/signals` instance;
- web server and serialization artifacts import `seroval` and
  `seroval-plugins/web`;
- declarations reach `csstype`, `seroval`, Solid, and renderer-local files;
- `@solidjs/web` consumes the installed peer instance of `solid-js`;
- nested `@solidjs/web` artifacts import other web subpaths whose condition
  resolution must also be recorded.

The ecosystem benchmark established that its temporary installations contained
the exact RC.3 trio and passed target-integrity checks. It did not retain the
installation lockfile, exact transitive versions and integrities, resolved
runtime and declaration closure, generated contracts, or review plans. The
temporary output directories named by its reports no longer existed at audit
time.

An exact isolated install lock and resolved-closure record are therefore
required. It is unsafe to infer RC.3 closure from the existing RC.0 runtime lock
or from current registry range resolution.

## Existing RC.3 evidence and its limits

Strong existing evidence included:

- `solid-rc3-evidence-review.md`, whose representative hashes and runtime
  analysis were correct except for its `gitHead` statement;
- `scripts/ecosystem-benchmark/manifest.json`, which recorded
  `auditedSolid2: 2.0.0-rc.3`, the three correct SRIs, exact RC.3 probe tuples,
  dist tags, and release histories;
- benchmark results recording the exact installed trio and
  `integrityVerified: true`.

That evidence does not amount to complete RC.3 certification. The generated
results explicitly recorded `fullyProven: false`:

- `solid-js`: 82 exports, 52 exports with unknown knowledge, and 62 unknown
  domain instances;
- `@solidjs/signals`: 61 exports and 26 exports with generated unknown
  knowledge;
- `@solidjs/web`: 515 generated export instances and 64 exports with unknown
  knowledge.

The verification report additionally showed:

- `solid-js` was refused with 16 blockers: 91 passed claims were not written
  into the contract, 32 claims were undriven, and 15 incompleteness items
  remained;
- `@solidjs/web` retained 204 undriven claims and refused `.`, `./frames`, and
  `./server-functions`;
- `@solidjs/signals` retained 7 undriven claims.

These are valuable Phase 0 refusal and incompleteness observations. They are not
proof that the packages' behavior is completely modeled.

## Current RC.0 versus intended RC.3 authority

At audit time, all of the following remained RC.0-based:

- the three bundled Solid 2 contracts;
- `pkg/contracts/bundled/runtime-lock.json`;
- `rust/target/registry-integrity.json`;
- TypeScript oracle pins and the installed oracle packages;
- the Solid 2 example app and its lockfiles;
- dialect review contracts and export indexes;
- numerous dialect facts, source-discovery branches, rule implementations,
  fixtures, and rule pages;
- `rust/ARCHITECTURE.md`;
- the workspace Solid 2 skill.

Documents that name RC.3 as the audited runtime tuple therefore describe the
new published-artifact authority or the ecosystem benchmark target. They must
not imply that existing checker facts and bundled contracts have already been
migrated and revalidated.

The minimal core runtime-authority tuple is the three packages audited here.
Other official `@solidjs/*` RC.3 packages become independent contract subjects
when an analyzed entrypoint or its exact dependency/import closure reaches
them. Sharing a source commit or release suffix does not automatically include
them in the core tuple.

## Durable replayable evidence required

The Phase 0 audit should durably preserve:

1. raw registry version metadata for each exact package, including SRI, SHA-1,
   tarball URL, count, unpacked size, signatures, attestation URL, `gitHead`,
   repository, and repository directory;
2. each tarball's SHA-256 plus complete extracted-file SHA-256 manifest;
3. the raw published `package.json` bytes;
4. ordered export maps and concrete-target existence results;
5. runtime resolution traces for every supported condition and loader set;
6. independent TypeScript resolution traces for relevant NodeNext, Bundler,
   and custom-condition configurations;
7. an exact isolated-install lockfile and package-instance inventory;
8. the resolved runtime and declaration import closure, including explicit
   unbounded frontiers;
9. generated RC.3 contracts, review plans, probe reports, and evidence written
   back into proposal contracts;
10. a refusal report separating unknown, undriven, failed, and passed-but-not-
    written claims;
11. registry signature and provenance verification results, rather than copied
    signature metadata alone;
12. the exact commands, tool versions, condition sets, environment, and hashing
    algorithms needed to reproduce every result.

The tarballs need not be committed when they can be fetched by exact recorded
URL and accepted only after SRI, SHA-1, and SHA-256 verification. They should be
cached for replay and never trusted before those checks.

## Required replay sequence

1. Fetch and retain exact-version registry metadata.
2. Download only the tarball URLs contained in that retained metadata.
3. Verify SRI and registry SHA-1 before extraction.
4. Verify safe archive paths, regular-file-only contents, count, and unpacked
   size.
5. Emit complete sorted file-hash manifests.
6. Validate every concrete export target.
7. Exercise ordered runtime resolution for every supported condition set.
8. Trace declaration resolution independently with TypeScript.
9. Install the exact trio in an isolated directory with scripts disabled and
   retain the lockfile.
10. Record every resolved dependency and peer instance plus each imported
    runtime and declaration artifact.
11. Generate and probe with durable output directories and write verified
    evidence into the proposal.
12. Verify with caches disabled and preserve raw machine reports.
13. Verify registry signatures and provenance.
14. Compare the replayed results with this audit and refuse on any identity,
    resolution, closure, or claim-count mismatch.

The published RC.3 artifacts are internally consistent. The Phase 0 problem was
not artifact ambiguity; it was the repository's incomplete preservation of the
evidence and its mixed RC.0/RC.3 authority state.
