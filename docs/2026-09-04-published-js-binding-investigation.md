# Published JavaScript dependency-binding investigation

Date: 2026-09-04. Branch: `codex/phase19a-authenticated-proof-policy`.
Base: `80d2a81e97fcdf38fe329f7896ebeb6cc9cd5501`.

## Conclusion

Implementation follow-up: [ADR 0010](adr/0010-declaration-imports-and-runtime-closure.md)
records the subsequently requested declaration-file fix and its new JS probes.
The findings below describe the pre-fix investigation.

Neither original binding refusal establishes a missing published export or a
missing installed dependency. The immediate blockers belong to the checker's
evidence model. The two packages need different work:

- Kobalte 0.9.2 needs authenticated dependency composition. The existing graph
  path gets past the initial `Key` refusal, but its transitive `solid-js/web`
  proposal refuses on an unknown runtime export kind for `Aliases`.
- Kobalte 2.0.0-alpha.0 already certifies its JavaScript roots with open
  `creates`. Its sole external closure edge is a declaration import of `JSX`;
  the current closure model treats that edge as a hazard affecting every
  behavioral domain despite the JavaScript bundle having no module imports.

No production semantics, sandbox policy, pins, fixtures, snapshots, benchmark
reports, or phase20/21 ledgers change in this investigation. The earlier
probe-source fixture changes remain in the worktree. ADR 0009's decision to
retain the source-case refusal is unchanged. The accessor census is out of
scope.

## Reproduction and scope

Reuse the exact installs, Bun locks, and package integrities retained by ADR
0009's before run. There is no package installation or network acquisition.
The actual certifier runs use the existing pinned debug checker, the local Type
Facts producer, and the integrity-checked registry cache. An injected `fetch_`
throws on any cache miss; the checked run asserts **zero cache misses**.

Scratch report root:
`/private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/389877ba-d8a8-4f5e-9628-89e210df2471/scratchpad/probe-ts/`.

Commands run from the repository root:

```sh
node /private/tmp/probe-ts-binding-investigation.mjs
node /private/tmp/probe-ts-graph-investigation.mjs
node /private/tmp/probe-ts-types-investigation.cjs
SOLID_CHECKER_NATIVE_BIN=$PWD/rust/target/debug/solid-checker-rust \
SOLID_TYPEFACTS_BIN=$PWD/bin/solid-typefacts \
SOLID_CHECKER_REGISTRY_CACHE=$PWD/rust/target/registry-cache \
bun /private/tmp/probe-ts-certify-js-investigation.mjs
```

The last script calls the real `certifyContract`, selects only entrypoint `.`,
enables `--dependency-graph-lane`, supplies the existing recipe corpus, and
writes its catalogs and audits under `js-investigation/`. It uses the retained
scratch issuer configurations. It asserts the exact refusal for 0.9.2 and the
absence of domain-exhaustiveness demands for the certified alpha roots. These
are diagnostic reproductions that intentionally preserve today's outcome,
not assertions that the blockers have been fixed.

The first certifier run established the next refusal; a second run added the
cache-miss counter and outcome assertions. Both agreed. The final log is
`/private/tmp/probe-ts-certify-js-investigation-checked.log`; the final audits
are `js-investigation/0.9.2-checked.audit.json` and
`js-investigation/2.0.0-alpha.0-checked.audit.json`.

These root-only runs answer the dependency question. They are **not** a new
before/after comparison for the original 43 source/JS candidates, and are not
comparable to the checked-in benchmark without a recipe corpus.

## Kobalte 0.9.2: Key exists; the graph reaches a different refusal

`dist/index.js:3` and `dist/index.d.ts:3` both re-export `Key` from
`@solid-primitives/keyed`. The retained lock selects version **1.5.3**. Direct
`resolvePackageArtifacts` on that dependency binds `Key` successfully on both
axes:

| artifact | SHA-256 |
| --- | --- |
| `@solid-primitives/keyed/dist/index.js` | `91464fbcfb2b60d2026323438da21ab74bc9756dedcc314e8f35b05895f341b7` |
| `@solid-primitives/keyed/dist/index.d.ts` | `88b1085466b37cf3132e3034aadef10201ebcdd54c8fc5599c71b278249d1729` |

The original root resolver received no accepted dependency map. In
`packages/cli/scripts/artifact-resolution.mjs`, `bindExport` calls
`acceptedExternalBinding` and refuses when that map has no binding. Its error
text, `accepted dependency ... has no exact runtime binding`, does not mean
that a dependency was already accepted or that the package lacks the export.
Raw resolver output and a lock match are acquisition evidence, not acceptance.

The original benchmark command did not enable `--dependency-graph-lane`.
Enabling it in a root-only certification advances to:

```text
artifact-or-demand-planning refused: no certifiable artifact case; 1 case(s) refused; first refusal: ./web: solid-checker-rust: emit package contract: entry file <package-root>/web/dist/web.js exports "Aliases", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback
```

This is `solid-js@1.9.14`, reached through the dependency graph. The graph
prepares whole export surfaces; `@solid-primitives/keyed` has a runtime import
of `solid-js/web`, whose declaration surface also re-exports Solid APIs.
`Aliases` is initialized in `web/dist/web.js:26` with
`Object.assign(Object.create(null), {...})`. The published
`web/types/client.d.ts:2` declares `Record<string, string>`.

As corroboration, a local TypeScript program over the real files reports
runtime `Aliases` as **any**, but the declaration as **Record<string, string>**.
The authoritative native refusal is the output above; this TypeScript
inspection is not a replacement Type Facts witness. The producer's
`callabilityOfType`/`constructabilityOfType` preserve Unknown for any, and
`promote_entry_callable` in `rust/crates/solid-facts-backend/src/main.rs`
correctly refuses to turn that into a non-callable value claim. Trusting the
declaration instead would restore the unsoundness that this check prevents.

## Alpha: an authenticated type dependency is not yet a discharged closure hazard

The installed `@solidjs/web@2.0.0-rc.0` satisfies the package's exact peer
declaration. Bundler-mode TypeScript resolution locates its published
`types/index.d.ts`. Kobalte's `dist/index.d.ts:1` imports `JSX` from it.
An AST walk of Kobalte's `dist/index.js` finds **zero** static import/re-export,
dynamic import, or require expressions. The resolver reports exactly one
external hazard: `./dist/index.d.ts:@solidjs/web`.

Both JS root cases certify, each with ten function exports and **zero closed
creates domains**. There are no domain-exhaustiveness demands and no withheld
closure candidates in this root-only run. No creates probe is scheduled.
Successful receipt publication here is not proof of a closed creates domain.

`closureForRoots` in the JS adapter and `record_external`/
`record_opaque_frontier` in the Rust module-closure replay both turn an external
declaration edge without an accepted semantic dependency into a hazard over
all behavioral domains. `acquireRootCompilerSources` already supplies
authenticated declaration snapshots for Type Facts; that source evidence does
not discharge this earlier closure hazard. The graph option does not change
this successful, open-domain root proposal into a dependency-composition
refusal eligible for graph preparation.

## Avoid confusing the two graph implementations

The standalone `discoverInstalledPublishedGraph` helper separately refuses
0.9.2 on declaration-only `csstype@3.2.3`, and alpha on nonliteral loading in
`@solidjs/web/dist/web.js:39332..39348`. The benchmark's planning-only graph
also reports a missing runtime `solid-js/types/reactive/signal.js`; TypeScript
correctly resolves the declaration specifier to `signal.d.ts`.

Those are useful evidence of the helpers' runtime/declaration conflation, but
**they are not the actual certifier's terminal failures**. The certifier's
`preparePublishedGraphFallback` uses `externalDependencies.axis` and the shared
compiler-source collector. The actual offline certification above supersedes
those helper errors when deciding the next implementation slice.

## Recommended next slices

Start with **alpha's declaration-only closure evidence**. Specify how the
verifier derives an erased/declaration edge from authenticated source and binds
its exact target/lock/conditions using the existing compiler-source channel.
Keep that evidence separate from executable dependencies and semantic receipt
authority. Update generator and Rust replay together; never simply remove
hazards from the proposal. A declaration can support type resolution without
claiming that its runtime implementation has been certified.

Required paired regressions: a declaration-only JSX dependency does not import
its runtime into the probe; a real value import still requires authenticated
runtime dependency evidence; missing/tampered/wrong-version declaration inputs
still refuse the demands that need them. New JS creates candidates need their
own recipes, implementation census, and mandatory veto. Their number remains
unmeasured until that change exists.

For 0.9.2, investigate **runtime export-kind evidence** for the exact
`Object.create(null)`/`Object.assign` initialization, including intrinsic
identity and mutation premises. A second option is a verifier-enforced
dependency projection that proves only required export bindings while retaining
the full executable module closure. Neither option may silently label unknown
exports as values, trust their `.d.ts`, or omit executable dependencies.

No package-author change is justified by these observations. No source case
became probeable, no creates gate completed, and no contradiction was observed
in this investigation. Source-case closure still cannot transfer from a JS
sibling. The original `exportsProven` benchmark measurement remains zero; this
investigation did not rerun that benchmark metric.

## Validation

The local binding, type/AST, and actual offline certification reproductions
completed with their assertions passing. No Cargo command was run, so the
compiled certification pins were not disturbed. This documentation-only
follow-up does not repeat the broad verification pass already recorded in ADR
0009. `git diff --check` covers the documentation changes. No snapshot update,
full ecosystem benchmark, `make verify`, commit, or push was performed.
