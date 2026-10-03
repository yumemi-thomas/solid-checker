# Feedback precision and existing application configuration

This slice completes the bounded milestone: correct the demonstrated native
overclaims, run the client collector with its existing application configuration,
and reproduce a retained real integration failure with source evidence.
It does not replace certification or establish a general warning accuracy rate.

## Native precision

The unchanged `oscartbeaumont-website` invoicer's fourteen SC2003 claims disappear.
Its `props.state` is an application-owned writable proxy; a fact about the outer
props container does not establish readonly behavior for the nested value.
`upstream_compat/shared_reactivity.rs` now requires a direct property write to
the exact props container. Transparent TypeScript wrappers are preserved, while
a wrapper around a nested member cannot masquerade as that container. Known
store sources retain their independent proof.

The unchanged `helge-dev` NavBar SC1001 becomes uncertifiable. Creating a
function-valued JSX prop does not prove invocation during rendering.
`execution_role.rs` examines the attribute's exact expression and function span;
`findings.rs` describes the unresolved consumer invocation context. It does not
guess timing from `onClick` or a component name.

| Application | Previous native findings | Current native findings | Current gaps |
| --- | ---: | ---: | ---: |
| helge-dev | 2 | 1 | 5 |
| oscartbeaumont-website | 17 | 3 | 11 |

An independent comparison matches exact diagnostic locations and unchanged
application input pins: fourteen removed claims and one downgraded claim.
The four remaining findings comprise two unvalidated SC5003 loading-context
warnings and two SC8015 preferences. Both installed TypeScript configurations
pass. No new active application defect is claimed.

`Props.tsx` adds positive direct-props/eager-read controls, negative nested-value
and wrapped-value controls, and uncertifiable direct/wrapped callback-prop
reads. These cases pass TypeScript 5.9.3 with the real published Solid/signals/web
RC.9 declarations. The product-owned nested-props case also passes the real
typing ownership gate. The documented upstream revision lookup returned 404;
this decision follows local runtime evidence and the product precision contract,
and makes no new upstream parity claim.

## Collector integration

`feedback-browser.mjs` loads the original Vite configuration and records its
configuration dependencies. Application compiler plugins, aliases, assets,
root and base are preserved; a default compiler is loaded only when no config
exists. Capture hooks precede application `pre` compiler transforms. Dependency
optimization stays disabled for the reviewed native reader.

Failed readiness steps now retain earlier assertions, browser exceptions,
bounded console diagnostics, read records and the failing selector. Later
interactions stop. Unmapped reads retain their runtime frames without claiming
authored source attribution.

The actual CLI runs the unchanged helge application with its original
`vite.config.ts`; no source/config overlay or replacement application supplies
the result. Four scenario assertions pass, with no page errors, console
warnings/errors, blocked requests or native runtime diagnostics. The single
live run takes 12.97 seconds and approximately 761 MB peak process-tree RSS.
Other gates ran concurrently, so these are observations, not a cost guarantee.
There are 879 read entries, 247 observer queries, six retained records and no
dropped records. All six retained records are unmapped; zero admitted
observations/automatic notes does not establish warning precision.

## Retained failure with source evidence

Fresh execution of finds-team's unchanged opt-in Kobalte hydration integration
passes its published typings and fails its existing hydration/clean-console
prerequisites. The native control passes. The failing test's body is not reached;
this does not demonstrate a later button interaction defect.

The independent source audit verifies byte identity between the production
client actually served and the source-mapped build, plus equality between
embedded sources and installed package bytes. It admits 22 exact frames and
one informational failure note; the native control receives none. Two hydration
errors, two halted-reactivity diagnostics and four CSP diagnostics are retained.
The missing-node exception is at `@solidjs/web/dist/web.js:1414`; the scheduler
reports the halted reaction at `@solidjs/signals/dist/prod/core/scheduler.js:344`.
The note also links the actual test and its failed prerequisite lines 15 and 19.
These are useful debugging locations, not proof that the file-owning package
caused the integration mismatch. CSP document locations remain outside the
client source map. No repair is proposed or certified.

The test-assisted path remains a retained application-test workflow. The
client-only CLI does not acquire general SSR support from this replay.

## Validation and artifacts

- IR library: 301 tests passed.
- Armed backend diagnostic process suite: 17 tests passed, including the new
  finding-kind/source-location regression.
- CLI suite and type adapter check: 334 tests passed and typing check passed.
- Ownership gate: 42 cases passed; migration ledger 465 rows, none pending.
- Coverage: 142 fixtures and 750 findings match. The non-updating comparison
  identified exactly two snapshots, then only those were patched: the new Props
  findings and the consumer-invocation wording in `retired-1x-spellings`.
- Rust formatting, workspace/all-targets Clippy with certification pins,
  `git diff --check`, schema JSON and dialect manifests passed.
- Independent precision comparison and hydration source audits passed.

The initial build in the existing debug directory stalled. A
fresh ignored Cargo target directory, without incremental/debug data, completed
the tests and pinned build. The checker used for every final native gate is
`rust/target/feedback-next-milestone-build/debug/solid-checker-rust`; the packaged
binary was not overwritten. Interrupted/intermediate trials are not final
validation evidence. No contracts, schemas or dialect manifests changed.
Full `make verify`, the feature matrix, contract corpus and performance
certification were deferred for this scoped precision/adapter slice.

Final retained artifacts under `rust/target/`:

- `feedback-next-milestone-native-evaluation-20261003-v3/results.json` and
  `helge-dev.live.json`: completed unchanged-source CLI replay and input pins.
- `feedback-next-milestone-precision-audit-20261003-v2.json`: exact-site comparison.
- `feedback-next-milestone-hydration-audit-20261003.json`: independent failure
  and source evidence, with exact installed source paths.
- `feedback-next-milestone-hydration-{candidate,native}-20261003/results.json`
  and corresponding `*-observed-*` directories: original tests and byte maps.
- `feedback-next-milestone-coverage-match.log`, `*-ownership.log` and
  `*-universal.log`: completed gate results.

Still open: unknown callback invocation timing, nested prop-value behavior
without an independent source proof, the six unmapped client records, the two
unvalidated loading warnings, the larger app's analysis timeout, SSR adaptation,
causal package attribution and repair safety. The initial three-application
evaluation remains the source for the timeout; the final focused replay does
not rerun that unchanged performance question.
