# runtime-oracle

Cases for `scripts/runtime-oracle.mjs`, which runs each one in Chrome on the
Solid 2 dev build and compares the runtime's own diagnostics with the
checker's verdict at the same site. It measures unproven clean claims
directly: a dev diagnostic where the checker reports nothing is a finding the
checker is missing, which no corpus A/B by finding key can see.

```sh
SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/release/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
node scripts/runtime-oracle.mjs --out <fresh.json> [--only id,...]
```

By default the cases run inside `../solid-checker-rc13-corpus/apps/helge-dev`
(solid-js, @solidjs/web and @solidjs/signals 2.0.0-rc.13, vite,
@solidjs/vite-plugin, playwright) and the newest playwright Chromium. Pass `--install /absolute/path/to/solid-checker-rc13-corpus/apps/helge-dev`
when running from a worktree without the default sibling corpus. The expanded
review archive takes several minutes at `--concurrency 3`.

## A case

A directory with an `App.tsx` and any other sources. These directories carry
no `tsconfig.json`, so `coverage.mjs` does not pick them up. Every exported
component is rendered in its own root, and the entry then dispatches
click, keyboard, focus, pointer and form events on every element in two passes,
so most cases need no scripted steps. Type stubs (`*.d.ts`) and
`node_modules` are not copied: a case runs against the real packages, and a
case `tsc` rejects is reported as `type-error` and proves nothing.

`oracle.json` is optional:

- `note`: what the case pins;
- `steps`: extra `feedback run` scenario steps (`assert-text` requires a unique
  `id`, as in the feedback scenario schema);
- `install`: optional corpus app directory, resolved relative to the parent of
  `--install` (the corpus apps root). It must have the same exact Solid version
  as the default install. Tooling still comes from `--tooling` or the default
  install. Omitting this field preserves the existing behavior;
- `open`: known unproven clean sites, each `{ code, site: "File.tsx:line", reason }`.
  They are debt, not passes. A listed site that stops firing fails the run as
  `stale-open`, so a fix updates the list.

## Verdicts

Per runtime diagnostic in a case file, against the checker rules that claim
its code (`codeRules` in the script):

- `covered-violation` or `covered-uncertifiable`: a finding of that kind sits on
  the diagnostic's line or on an application frame of its stack (the
  collector's `stack` field), because a finding may anchor at the call that led
  to the read rather than at the read itself;
- `unproven-clean`: nothing there. The run exits 1;
- `known-open`: an `unproven-clean` listed in `open`;
- `no-rule`: a code no rule claims (`REACTIVITY_HALTED`).

`static-only` means the case has checker violations but no attributed runtime
diagnostics. Inspect execution before calling it a false positive: some runtime
defects have no dev diagnostic. Individual `staticOnly` rows also occur in
otherwise covered cases when the runtime attributes a prop getter to its caller
and the finding matches an application stack frame instead of the primary site.
`STRICT_READ_UNTRACKED` is matched against both `strict-read-untracked` and its
`no-destructure` special case. Collector failures and feedback reports missing
analysis or execution are `harness-error`, never `quiet`.

## Sources

The `c*` and `r*` cases are the reviewer counterexamples from the ADR 0319 to
0325 rounds (copied from `/tmp/adr319-review.noindex`, preserved in full under
`rust/target/research/session-2026-10-19/`). Two were edited only to
type-check against the real typings: `c3-misc` (`run(undefined)`) and `r4b`
(`(String(), g)()`). The `oracle-*` cases are minimal probes for the families
the first run found, both closed by ADRs 0326 and 0327; `oracle-owner-flow-owned`
is the owned control for the second. `control-body-read` is a known-detected
control.

The 2026-10-19 port adds the remaining archived ADR 0319–0325 rounds from
`/Users/thomas/Documents/Github/solid-checker/rust/target/research/session-2026-10-19/adr319-review.noindex/`
and all seven ADR 0326/0327 review rounds from
`/tmp/sc-scratch.noindex/review-5570a4fad/cases`, then
`review-r2/cases` through `review-r7/cases`.

| Source | Added cases | Prefix |
| --- | ---: | --- |
| archived r5 | 16 | `r5-` |
| archived r6 | 195 | `r6-` |
| archived r8 | 32 | `r8-` |
| archived r9 | 171 | `r9-` |
| archived r10 | 33 | `r10-` |
| archived r11 | 16 | `r11-` |
| review-5570a4fad (round 1) | 10 | `review-r1-` |
| review-r2 | 14 | `review-r2-` |
| review-r3 | 9 | `review-r3-` |
| review-r4 | 8 | `review-r4-` |
| review-r5 | 5 | `review-r5-` |
| review-r6 | 11 | `review-r6-` |
| review-r7 | 4 | `review-r7-` |
| **Total** | **524** | |

Repeated archive reruns with identical complete source sets share one case;
different helper sources remain separate, even when App.tsx is identical.
The r6 and r9 `//@@` split lists are fully represented. r5 also pins the
seven saved `lib.event/deferred/tracked/body/1/2/3.ts` source-edit variants as
independent cases. This oracle measures their cold runtime behavior, not
retained-session cache invalidation.

The r10 cases (and r5/t11) select `install: "helge-dev"`, which has real
`@solidjs/router@2.0.0-next.26` and Solid rc.13. They mount the subject on a
`/:id/:a/:b` route and include a navigation click. Existing router/history
configuration and mutations remain in the subject. RouterComponentProp uses
the published `createRouter` provider API instead of the obsolete `Router`
export.

Each case's `note` records adaptations. Components requiring props have a
small mount wrapper passing populated signal-backed props, so missing input
does not suppress reads or branches. Original ambient inputs retain their
declarations; a runtime-only global witness supplies a no-op sink/runner,
synchronous callback consumer, true flag, and resolved promise. These are
explicit test inputs, not inferred package contracts. The r8 synthetic
`dep-ns/widget` JavaScript is copied into typed `widget.ts` with its original
runtime behavior; those cases exercise local invocation rather than the
original dependency namespace/source-selection premise. Solid and JSX always
use the real installed declarations. JSX types come from `@solidjs/web`.

Typing edits preserve behavior where possible: `on:click` becomes `onClick`
(rc.13 rejects the former with TS2322), and duplicate literal keys put the
first key in a spread (TS1117), preserving last-property-wins behavior.
Other notes identify the required JSX callback return type, a void effect apply
that starts the async continuation, the current root options argument, nonzero
`Show` callback parameters, and a valid DOM attribute alongside an otherwise
weakly typed spread.
Quiet cases are runtime controls, not proof that every branch executed.

### Skipped archive cases

- r5 `t4/app`, `t5/app`: workspace package boundaries and source-selection
  metadata require their original package install; a local import would erase
  the distinction being tested.
- r10 `PatchDirOnly`, `OverrideTarball`, `PatchPackage`, `ForkSameVersion`,
  `OverrideFork`, `NpmAliasName`, `YarnPatchProtocol`,
  `V_2_0_0_next_26_local_`, `V_2_0_0_next_26__`,
  `V_2_0_0_next_2_`, `V_2_0_0_next_260_`, `NoUseParamsOld`:
  package versions, aliases, forks and patched/overridden manifests cannot be
  represented by source copied into the audited install. Their shared App
  source is covered by `r10-ctl`; the metadata counterexamples are not.
- r10 `PackageHistory`: its synthetic `fancy-history` package is not in the
  real install; substituting a local history erases the package boundary.
- r11 `CjsAmbientGlobal`, `CjsBracket`, `CjsExports`, `CjsGlobalExports`,
  `CjsGlobalModule`, `CjsModuleExports`, `DefineProp`: require CommonJS
  export objects. A browser ESM module has no such binding, and inventing one
  would not reproduce CommonJS export mutation.
- r11 `Declared`, `DtsOnly`: the exported value has declarations only and no
  runtime implementation. Supplying an implementation changes the premise.

The source archive and supplied binaries are unchanged. The harness generates
runtime copies and feedback reports under the selected corpus app's
`.solid-checker-runtime-oracle/`; the aggregate report goes to `--out`.

### Newly recorded open sites

The port adds 14 open sites in 13 cases. With the existing c2d sites and r4c
async-attribution mismatch, the suite carries 17 open entries. Each JSON entry
records its exact code, runtime site, and reason; none is a claim of safety.

| Family | Sites | Cases |
| --- | ---: | --- |
| Opaque callback/receiver timing under explicit runtime witnesses | 3 | `r6-b17-proparrowtoopaque`, `r6-b17-storereadarrowtoopaque`, `r6-b6-classmethodopaque` |
| Reactive route-parameter destructuring not established from dependency source | 1 | `r10-destructured` |
| Returned children/memo accessor read in a setup body | 2 | `r9-b3-childrenreadinbody`, `r9-d1-mixmemoslotreadinbody` |
| Async provenance through the children accessor | 1 | `r9-b3-childrenasync` |
| Mixed owned/event invocation refuses the owner proof (ADR 0327) | 2 | `review-r1-owner-mix` line 5, `review-r3-owner-double-call` |
| Handler returned from a root, then invoked without that owner | 1 | `review-r1-owner-mix` line 13 |
| Nested class-field, default-parameter, and JSX-getter owner flow (ADR 0327) | 3 | `review-r5-owner-class-control`, `review-r6-owner-default-control`, `review-r6-owner-jsx-getter-control` |
| Event tuple handler not followed by owner seeding | 1 | `review-r7-event-array` |

### Static-only investigation

Possible false positives are retained, without changing checker code:

| Family | Cases | Execution evidence |
| --- | --- | --- |
| Reads after await, outside the runtime strict-read window | `r9-d3-forawait`, `r9-d3-ifawait`, `r9-d3-loopawait`, `r9-d3-tryawait`, and the second read in `r9-d3-namedasyncnoawaitcall` | An assertion verifies a marker set after each read. The named case also has a covered diagnostic at its first, pre-await read. |
| Writes to plain function-valued props | `review-r1-alias-mutation`, `review-r2-alias-mutation`, `review-r2-computed-assignment`, `review-r2-nested-assignment` | Assertions render both the replacement and the saved child, proving the mutation and tracked saved-child invocation completed. |
| Writes to plain props/merge/omit fields | `r9-d5-writeplainprops`, `r9-d5-writemerge`, `r9-d5-writeomit`, `r9-d5-writeomitalias` | Assertions read back the assigned value 1; these writes actually succeed. |
| Destructuring in tracked and permitted untracked/event invocations | `r9-d9-mixdestructure`, `r5-t1-tracked` | The first asserts two completed calls (memo and untrack). The second records two event-time reads of the reactive prop with no observer or owner; its other declared invocation is a lazy memo. |
| Synchronous event dispatch inherits the component owner (ADR 0327's event-owner premise) | `review-r7-sync-scroll`, `review-r7-sync-child-scroll` | Assertions verify rendering completed after synchronous dispatch and the explicit owner check. The dispatched element stays detached, so the automatic passes cannot add an unowned dispatch. |
| Plain children treated as a reactive prop by a local component | `r8-nsbase`, `widget.ts:3` | Assert Frame's supplied child renders through Root. The Root read has no matching diagnostic or application stack frame; StaticMembers' separate signal read is covered. |

These are 18 possible-false-positive cases, including two otherwise covered
cases: the named async case and the namespace control. The oracle alone is not
a checker fix.

Two further static-only cases are **real defects with no dev diagnostic**:
`r9-e2-storedirectwrite` and `r9-e2-storeviewwrite` assert the store stays 1
after assignment of 3. The runtime silently drops the writes; the checker
correctly reports `no-direct-mutation`.

Other `staticOnly` entries in otherwise covered cases are attribution
differences: the finding is on an application stack frame, while the runtime's
primary site is the calling getter. Their diagnostics remain covered using the
same stack-frame matching described above.

Two exceptions are coverage gaps: in `r5-t1` and `r5-t1-body`, `data()` throws
`PENDING_ASYNC_UNTRACKED_READ` before the following props destructure executes.
The earlier read is covered, and the later `no-destructure` row is not evidence
of a false positive in this run.

Application-frame matches for the remaining static-only rows:

| Case | Native row | Runtime primary site |
| --- | --- | --- |
| `r6-b17-proparrowtoprojectcallee` | `App.tsx:21 SC1001` | `App.tsx:28 STRICT_READ_UNTRACKED` |
| `r6-b17-propdirectbody` | `App.tsx:18 SC1001` | `App.tsx:25 STRICT_READ_UNTRACKED` |
| `r9-b1-beforeawait` | `App.tsx:9 SC1001` | `App.tsx:16 STRICT_READ_UNTRACKED` |
| `r9-b1-computed` | `App.tsx:9 SC1001` | `App.tsx:16 STRICT_READ_UNTRACKED` |
| `r9-b1-ctl` | `App.tsx:9 SC1001` | `App.tsx:16 STRICT_READ_UNTRACKED` |
| `r9-b1-destructure` | `App.tsx:9 SC1003` | `App.tsx:16 STRICT_READ_UNTRACKED` |
| `r9-b1-nestedarrowincallback` | `App.tsx:9 SC1001` | `App.tsx:16 STRICT_READ_UNTRACKED` |
| `r9-b1-optional` | `App.tsx:9 SC1001` | `App.tsx:16 STRICT_READ_UNTRACKED` |
| `r9-b1-stdlibthenproject` | `App.tsx:9 SC1001` | `App.tsx:16 STRICT_READ_UNTRACKED` |
| `r9-b1-stdlibthenproject` | `App.tsx:9 SC1001` | `App.tsx:16 STRICT_READ_UNTRACKED` |
| `r9-b1-tryfinally` | `App.tsx:9 SC1001` | `App.tsx:16 STRICT_READ_UNTRACKED` |
| `r9-b2-nestednamedhelperdirect` | `App.tsx:10 SC1001` | `App.tsx:17 STRICT_READ_UNTRACKED` |
| `r9-d8-helperfunctiondecl` | `App.tsx:13 SC1001` | `App.tsx:20 STRICT_READ_UNTRACKED` |
| `r9-d8-helperinadmitted` | `App.tsx:13 SC1001` | `App.tsx:20 STRICT_READ_UNTRACKED` |
| `r9-d8-helperrecursive` | `App.tsx:13 SC1001` | `App.tsx:20 STRICT_READ_UNTRACKED` |
| `r9-d8-helpershadowed` | `App.tsx:14 SC1001` | `App.tsx:22 STRICT_READ_UNTRACKED` |
| `r9-e1-rootpropsread` | `App.tsx:14 SC1001` | `App.tsx:21 STRICT_READ_UNTRACKED` |
| `r9-e3-destructuredparamnamedprops` | `App.tsx:10 SC1003` | `App.tsx:19 STRICT_READ_UNTRACKED` |
| `r9-e3-wrapas` | `App.tsx:11 SC1001` | `App.tsx:18 STRICT_READ_UNTRACKED` |
| `r9-e3-wrapnonnull` | `App.tsx:11 SC1001` | `App.tsx:18 STRICT_READ_UNTRACKED` |
| `r9-e3-wrapsatisfies` | `App.tsx:11 SC1001` | `App.tsx:18 STRICT_READ_UNTRACKED` |
| `r9-e4-formapperpropread` | `App.tsx:3 SC1001` | `App.tsx:9 STRICT_READ_UNTRACKED` |

The two `r9-b1-stdlibthenproject` rows are distinct findings on the same line.

Full validation on 2026-10-19, with checker `checker-6a4940bc9` and the supplied
Type Facts producer: **561 cases, exit 0** — 107 covered-violation,
118 covered-uncertifiable, 303 quiet, 15 known-open, 18 static-only. There are
17 known-open diagnostic sites, and no unproven-clean, stale-open, type-error,
or harness-error. All 44 individual static-only finding rows are accounted for
above (18 possible false positives, 2 silent store defects, 2 unreached
destructures, and 22 application-frame matches).
