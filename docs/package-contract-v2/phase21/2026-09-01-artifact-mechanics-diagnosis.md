# Infrastructure / artifact refusal classes — diagnosis

Read-only diagnosis at `codex/phase19a-authenticated-proof-policy`, HEAD `9c7326f10958fa228e51ef0a37cade6078305a08` (verified).
No tracked file modified. No cargo build/test run.

Binaries used (as the benchmark invokes them):

```
SOLID_CHECKER_NATIVE_BIN=$PWD/rust/target/debug/solid-checker-rust
SOLID_TYPEFACTS_BIN=$PWD/bin/solid-typefacts    # buildinfo sourceDigest 54e44650…, buildId "dev"
```

Neither binary refused on build digests.

**Note on the pinned report:** `benchmarks/ecosystem/report.json` records
`checker.nativeBin = rust/target/release/solid-checker-rust`. The reruns here used the
debug build. Every row still refuses with the same class and (with one exception, row
15/16 below) the same reason string.

Reproduction artifacts in this directory:

| file | contents |
| --- | --- |
| `repro-importsmap.json` / `.md` | the 3 `@tanstack/solid-start-server` rows |
| `repro-rest.json` / `.md` | the other 14 rows |
| `repro-live-2.json`, `repro-live-3.json` | two extra runs of the live-session-identity pair |
| `all-refusals.txt`, `refused-attempts.txt` | extracted refusal census from the pinned report |
| `start-0.0.4.tgz` | the authenticated `@solid-primitives/start@0.0.4` archive (integrity verified against the manifest pin) |

Runner invocation (identical for every row):

```sh
bun scripts/ecosystem-benchmark/run.mjs --timeout 600 --attempt-certification --keep-temp \
  --probe '<id>' --json <scratch>/….json
```

The refusal text lives at `results[].certificationAttempt.reason` (not in
`artifactCaseRefusals`, which is the *proposal*-stage channel and is empty for most of
these rows).

---

## Per-row table

| # | probeId | reproduced | mechanism (evidence) | class | disposition |
| --- | --- | --- | --- | --- | --- |
| 1 | `@tanstack/solid-start-server@1.167.36\|solid1\|only` | yes | M1 — `@tanstack/start-server-core@1.169.31` `dist/esm/createStartHandler.js` does `import("#tanstack-router-entry")`; its own `package.json` `imports` defines only `#tanstack-start-server-fn-resolver` and `#tanstack-start-plugin-adapters`. Node: `ERR_PACKAGE_IMPORT_NOT_DEFINED`. | **c** | stays refused |
| 2 | `@tanstack/solid-start-server@2.0.0-rc.2\|solid2\|floor` | yes | M1, `start-server-core@1.169.26`, byte-identical `imports` map | **c** | stays refused |
| 3 | `@tanstack/solid-start-server@2.0.0-rc.2\|solid2\|head` | yes | M1, same | **c** | stays refused |
| 4 | `@tanstack/ai-solid@0.19.1\|solid1\|only` | yes | M2 — declaration-extension substitution. `@ag-ui/core`'s `exports` has **no `types` condition**; `declarationCandidate` tries `.d.ts` before `.d.mts` and binds `dist/events-JPFRVbr9.d.ts`, while TypeScript strips `.mjs` and lands on `dist/events-Bg2nO3O2.d.mts`. The two chunk files are byte-identical apart from their own `sourceMappingURL`. | **a** | M2 fix |
| 5 | `@tanstack/solid-db@0.2.40\|solid1\|only` | yes | M3 — `@tanstack/db@0.8.5` `dist/esm/index.d.ts` has `import * as IR from './query/ir.js'; export { IR };`. The compiler resolves `IR`'s declaration to the *module file* `dist/esm/query/ir.d.ts` (declaration "name" is the quoted module path). The suffix rule requires the declaration to sit in the entry declaration file. | **a** | M3 fix |
| 6 | `@tanstack/solid-query@6.0.0-rc.0\|solid2\|floor` | yes | M4 — `build/_tsup-dts-rollup.d.ts:57` `import type { JSX } from '@solidjs/web';` and `build/index.js:4` `import { createComponent } from '@solidjs/web';`, but `peerDependencies` is `{"solid-js": ">=2.0.0-rc.0 <3.0.0"}` only. `@solidjs/web` is absent from the layout; `tsc --noEmit` reports `TS2307`. | **c** | stays refused |
| 7 | `@tanstack/solid-query@6.0.0-rc.0\|solid2\|head` | yes | M4, same | **c** | stays refused |
| 8 | `@tanstack/solid-query-persist-client@6.0.0-rc.0\|solid2\|floor` | yes | M4; peers are `solid-js` + `@tanstack/solid-query`, `@solidjs/web` undeclared | **c** | stays refused |
| 9 | `@tanstack/solid-query-persist-client@6.0.0-rc.0\|solid2\|head` | yes | M4, same | **c** | stays refused |
| 10 | `@solid-primitives/local-store@1.1.4\|solid1\|only` | yes | M5 — default-export binding is internally inconsistent: `export_bindings.rs:505-513` sets `runtime_export = "default"` but `runtime_span` = the span of the *local identifier* in `export default createLocalStore;`. The producer reports `QueryName = node.Text()` at that span, i.e. `createLocalStore`. `type_facts.rs:2161` compares them. | **a** | M5 fix |
| 11 | `@solid-primitives/tween@1.4.1\|solid1\|only` | yes | M5 (declaration half) — `dist/index.d.ts` has `export default function createTween(...)` + `export { createTween };`. Snapshot replay binds `declarations_export = "createTween"`; TypeScript's symbol for that declaration is named `default` (verified with the TS API). `type_facts.rs:1857-1861`. | **a** | M5 fix |
| 12 | `solid-recharts@1.0.1\|solid1\|only` | yes | M6 — `solid-recharts` **ships a `node_modules` inside its own archive**: `dist/browser/node_modules/csstype/index.d.ts` (469 670 B, `sha256:0eed9868…`). `verify_snapshot_source_census` attributes sources by `rsplit_once("/node_modules/csstype/")`, so that path is charged to the *hoisted* `csstype@3.2.3` snapshot (894 969 B, `sha256:ac51dd7d…`). | **a** | M6 fix |
| 13 | `@solid-primitives/start@0.0.4\|solid1\|only` | yes | M7 — the authenticated tarball contains two members, `package/./dist/index.cjs` and `package/dist/index.cjs`, both 1 190 B with `sha256:a31ea4af…`; `Path::components()` folds `.` away so both canonicalize to `dist/index.cjs`. | **a** | M7 fix |
| 14 | `motion-solidjs@0.6.0\|solid1\|only` | yes | M8 — `framer-motion@12.43.0` `dist/es/dom.mjs` has both `export * from 'motion-dom';` and `export { delayInSeconds as delay } from 'motion-dom';`. ECMA-262 resolves the explicit indirect entry and discards the star candidate; Node agrees (`dom.delay === motionDom.delayInSeconds` → `true`). `main.rs:5262` treats them as equal-weight candidates. | **a** | M8 fix |
| 15 | `@tanstack/solid-query@5.102.5\|solid1\|only` | yes (reason string differs from the pinned report — see M9) | M9 — `TypeFactsCertificationError::IdentityMismatch`, one opaque variant covering ~14 distinct disagreements. | **d** | M9: make the variant diagnostic first |
| 16 | `@tanstack/solid-query-persist-client@5.102.5\|solid1\|only` | yes (same) | M9 | **d** | M9 |
| 17 | `@solidjs/testing-library@0.8.10\|solid1\|only` | yes | M10 — `@solidjs/router` is an **explicitly optional** peer (`peerDependenciesMeta["@solidjs/router"].optional = true`) reached only through a guarded dynamic `import()` inside `try { … } catch { … }` with a defined fallback. `findPackageRoot` (`artifact-resolution.mjs:283`) makes its absence fatal to the whole artifact case. | **a** | M10 fix |

**Reproduction: 17/17.** No row was excluded.

The task brief named ~16 rows across 7 buckets; the "authenticated-dependency-layout" bucket
(5 rows) is *not disjoint* — it is exactly rows 6-9 (M4) plus row 17 (M10). Unique rows: 17.

Class counts by row: **a = 8, c = 7, d = 2, b = 0.**
(No row is class b in the narrow sense of "an ambiguous artifact where refusing is the only
sound answer". The 7 class-c rows are nonetheless *correctly* refused today — see the notes.)

---

## Mechanism specs

### M1 — `#tanstack-router-entry` is not in the imports map (class c, 3 rows)

Not a subpath-pattern gap. The specifier is simply absent, and Node refuses identically.

`@tanstack/start-server-core@1.169.26` (and `.31`) `package.json`:

```json
"imports": {
  "#tanstack-start-server-fn-resolver": { "default": "./dist/esm/fake-start-server-fn-resolver.js" },
  "#tanstack-start-plugin-adapters":    { "default": "./dist/esm/empty-plugin-adapters.js" }
}
```

`dist/esm/createStartHandler.js`:

```js
async function loadEntries() {
	const [routerEntry, startEntry, pluginAdapters] = await Promise.all([
		import("#tanstack-router-entry"),
		import("#tanstack-start-entry"),
		import("#tanstack-start-plugin-adapters")
	]);
```

Two of the three are undefined. Node, from a probe file placed at
`<pkg>/dist/esm/` (node v24.11.1, `import.meta.resolve`):

```
FAIL  #tanstack-router-entry -> ERR_PACKAGE_IMPORT_NOT_DEFINED Package import specifier
      "#tanstack-router-entry" is not defined in package
      …/node_modules/@tanstack/start-server-core/package.json
FAIL  #tanstack-start-entry  -> ERR_PACKAGE_IMPORT_NOT_DEFINED
OK    #tanstack-start-plugin-adapters    -> …/dist/esm/empty-plugin-adapters.js
OK    #tanstack-start-server-fn-resolver -> …/dist/esm/fake-start-server-fn-resolver.js
```

The sibling package `@tanstack/start-client-core@1.170.22` **does** define all three,
with real fallback files:

```json
"imports": {
  "#tanstack-start-entry":           { "types": "./src/start-entry.d.ts", "default": "./dist/esm/fake-entries/start.js" },
  "#tanstack-router-entry":          { "types": "./src/start-entry.d.ts", "default": "./dist/esm/fake-entries/router.js" },
  "#tanstack-start-plugin-adapters": { "types": "./src/start-entry.d.ts", "default": "./dist/esm/empty-plugin-adapters.js" }
}
```

and ships `dist/esm/fake-entries/{router,start}.js`. `start-server-core` ships neither
those files nor those entries. The design intent (each core package defines fake entries
that the Vite plugin overrides) is visible in `start-client-core`; `start-server-core`
omits two of them.

**Class c, upstream packaging defect in `@tanstack/start-server-core`.** The refusal is
the right answer and must not be relaxed. Meets the `context@0.3.2` evidence standard
(the resolver of record — here Node rather than `tsc`, because the specifier is a runtime
`import()` — refuses the published bytes in an ordinary layout).

Reach: the same leaf also appears (as `dependencyPlan.leaves[].code = "package-import-not-defined"`)
in `@tanstack/solid-start@1.168.47|solid1|only` and both `@tanstack/solid-start@2.0.0-rc.2`
rows, which certify **partially**. Upstream fixing this clears 3 refusals and widens 3 partials.

---

### M2 — declaration-extension substitution (class a, 1 row) — **cheap, high confidence**

`packages/cli/scripts/artifact-resolution.mjs:33`:

```js
const DECLARATION_EXTENSIONS = [".d.ts", ".d.mts", ".d.cts"];
```

`declarationCandidate` (line 486) builds `[stem + each extension]` and takes
`candidates.find(isFile)` — **`.d.ts` first, regardless of the runtime target's module
format**. TypeScript does not do that. `tsc --traceResolution`, `moduleResolution: Bundler`
(the certification harness's own setting), on `@ag-ui/core@0.1.1-canary.beta.0`:

```
File name '…/@ag-ui/core/dist/index.mjs' has a '.mjs' extension - stripping it.
File '…/@ag-ui/core/dist/index.mts' does not exist.
File '…/@ag-ui/core/dist/index.d.mts' exists - use it as a name resolution result.
```

`index.d.ts` is never even considered. The package makes this reachable because its
`exports` map carries **no `types` condition at all**:

```json
"exports": { ".": { "require": "./dist/index.js", "import": "./dist/index.mjs" }, … }
```

so the declarations axis resolves the `import` condition to `dist/index.mjs` and then
substitutes an extension. The snapshot picks `dist/index.d.ts` → chunk
`events-JPFRVbr9.d.ts`; the live session picks `dist/index.d.mts` → chunk
`events-Bg2nO3O2.d.mts`. The two chunks are identical except for their own
`sourceMappingURL` line.

**Bounded fix.** Make `declarationCandidate` format-directed, mirroring TypeScript's
extension substitution:

| runtime target ends with | declaration candidates, in order |
| --- | --- |
| `.mjs` / `.mts` | `.d.mts` only |
| `.cjs` / `.cts` | `.d.cts` only |
| `.js` / `.jsx` / `.ts` / `.tsx` | `.d.ts` only |
| already a `.d.*` file | itself |
| extensionless / directory | `index.d.ts`, `index.d.mts`, `index.d.cts` (unchanged) |

Must-keep-refusing traps:

- **No cross-format fallback.** If the `.mjs` target has no `.d.mts` sibling, the
  declarations axis must fail (`declarations-not-found`) rather than silently borrowing
  `.d.ts`. Borrowing is exactly what produces a census that names bytes the live session
  never read.
- **Do not "accept either twin".** Relaxing the *suffix check* in `type_facts.rs` to
  tolerate a `.d.mts`/`.d.ts` pair would leave `declarations.path`/`declarations.sha256`
  in the emitted contract naming a file the compiler did not consult. The fix belongs in
  selection, not in the comparison.
- A `types`/`typings` condition, when present, still wins over extension substitution —
  do not reorder that.
- The Rust snapshot replay must select the same file; `declarations_path` and the
  `declarationsDigest` in the proposal are part of the census identity.

---

### M3 — namespace (module-object) exports (class a, 1 row)

`@tanstack/db@0.8.5` `dist/esm/index.d.ts`:

```ts
import * as IR from './query/ir.js';
…
export { IR };
```

The compiler's value declaration for `IR` is the *source file* `dist/esm/query/ir.d.ts`;
its declaration "name" is the quoted module path, which is why the refusal reads
`resolved value declaration "\"/…/dist/esm/query/ir\"" at "/…/dist/esm/query/ir.d.ts"`.

`type_facts.rs:1827-1836` requires `actual_path.ends_with(marker)` where `marker =
/node_modules/<pkg>/<declaration_path>` and `declaration_path` is the *entry* declaration
file. A namespace export's declaration necessarily lives elsewhere in the package.

An escape hatch already exists for the *dependency* direction —
`authenticated_dependency_declaration_target` (`type_facts.rs:1863-1885`) accepts a
declaration that lands anywhere inside an authenticated **dependency** snapshot. There is
no equivalent for the node's *own* snapshot.

**Bounded fix.** Accept a resolved declaration whose path is a member of *this plan's own*
authenticated snapshot, when the export's snapshot-replayed binding is a module-namespace
binding:

1. the snapshot replay recorded the export as an `import * as N` / `export * as N`
   namespace binding (i.e. `bind_export` followed a namespace import, `name == "*"` at
   `export_bindings.rs:572`);
2. the compiler's declaration path, after stripping the plan's package marker, is a key in
   `plan.snapshot.files` — the exact member, not "some file in the package directory";
3. that member is the same module the replay's namespace binding names.

Must-keep-refusing traps:

- A **non**-namespace export whose declaration escapes the entry declaration file must
  still refuse — that is the genuine "the compiler went somewhere the snapshot did not
  select" signal.
- The member must be in the authenticated snapshot's own `files` map; a path that merely
  sits under the package *directory* (e.g. a vendored copy, see M6) is not the same thing.
- Do not treat "the declaration name looks like a quoted path" as authority on its own.
  Require the namespace binding on the replay side.
- Do not fold this into a blanket "any member of my own snapshot is fine" — that would
  also swallow M2, hiding a real selection defect behind a relaxed comparison.

---

### M4 — `@solidjs/web` is an undeclared peer (class c, 4 rows)

`@tanstack/solid-query@6.0.0-rc.0` `package.json`:

```json
"peerDependencies": { "solid-js": ">=2.0.0-rc.0 <3.0.0" },
"dependencies":     { "@tanstack/query-core": "5.101.0" }
```

`build/index.js:4`:

```js
import { createComponent } from '@solidjs/web';
```

`build/_tsup-dts-rollup.d.ts:57`:

```ts
import type { JSX } from '@solidjs/web';
```

`@tanstack/solid-query-persist-client@6.0.0-rc.0` is the same shape
(`peerDependencies: { "solid-js": …, "@tanstack/solid-query": "^6.0.0-rc.0" }`,
`_tsup-dts-rollup.d.ts:3` imports `JSX` from `@solidjs/web`).

The benchmark installs a probe's Solid pins plus the package's own declared peers, so the
tree is `{@tanstack/solid-query, solid-js, @solidjs/signals, csstype, seroval…}` — no
`@solidjs/web`. Published-typing oracle in exactly that layout:

```
packages/cli/node_modules/.bin/tsc --noEmit -p <oracle>/tsconfig.json
…/@tanstack/solid-query/build/_tsup-dts-rollup.d.ts(57,26): error TS2307:
Cannot find module '@solidjs/web' or its corresponding type declarations.
```

**Class c**, and the absolute rule applies directly: TypeScript already reports this, so
the checker must not manufacture a finding. Refusing is the correct answer for this
layout.

The mention of `_tsup-dts-rollup.d.ts` in the reason is **incidental** — it is only the
importer whose resolution failed. There is no declaration-rollup selection rule involved
and no rollup shadowing a per-entry declaration. The task brief's hypothesis for these
four rows does not hold.

Separately (a benchmark-scope question, not a checker fix): a real Solid 2 application
installs `@solidjs/web`, so these packages work in practice. Adding `@solidjs/web` to the
solid2 probe pins would change what is being certified (the certified artifact case would
then include the renderer's declarations) and should be a deliberate manifest decision,
not a way to clear four rows.

---

### M5 — default-export identity is modelled two ways (class a, 2 rows) — **cheap**

Two halves of one inconsistency, both in the checker.

**Runtime half (`local-store`).** `export_bindings.rs:500-514`:

```rust
ExportKind::Default => {
    if !export.type_only {
        description.direct.insert(
            "default".into(),
            BindingTarget {
                file: path.into(),
                name: "default".into(),                          // <-- always literal "default"
                snapshot_root: self.snapshot.root().into(),
                span: export.declarations.first()
                    .map(|declaration| declaration.local.span),  // <-- span of the LOCAL identifier
            },
        );
    }
}
```

`bind_export` returns this target unchanged (`direct.file == path`, line 572), so
`runtime_binding("default")` yields `runtime_export = "default"` with a span pointing at
`createLocalStore` in `export default createLocalStore;`. The producer
(`apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts.go:208`) reports:

```go
transcript.QueryName = node.Text()
```

i.e. `"createLocalStore"`. `type_facts.rs:2161` then compares:

```rust
if implementation.query_name.as_ref() != runtime_export || !actual_path.ends_with(&expected_suffix)
```

The two fields are inconsistent *by construction*: whenever a default export's operand is
a named identifier, this comparison cannot succeed. `@solid-primitives/local-store@1.1.4`
is a minimal instance — one export, `export default createLocalStore;` over a plain
`function createLocalStore(...)`.

**Declaration half (`tween`).** `@solid-primitives/tween@1.4.1` `dist/index.d.ts`:

```ts
export default function createTween(target: () => number, { ease, duration }: TweenProps): () => number;
export { createTween };
```

The replay binds the public export `createTween` from the `export { createTween }`
specifier, so `declarations_export = "createTween"` (not `"default"`, so the default
short-circuit at `type_facts.rs:1843-1855` is skipped). TypeScript's symbol for that
declaration is named `default`. Verified with the TypeScript 5.9.3 compiler API against
the installed package, `moduleResolution: Bundler`:

```json
{"importedAs":"createTween","aliasSymbolName":"createTween","targetSymbolName":"default",
 "targetDeclKind":"FunctionDeclaration","targetDeclName":"createTween",
 "targetDeclFile":"…/@solid-primitives/tween/dist/index.d.ts"}
```

`actual_path` matches the marker; only `actual_name != declaration_export` fails
(`type_facts.rs:1857-1861`).

**Bounded fix.** Give a default-exported declaration one identity on both sides.

1. In `export_bindings.rs`, when a named specifier's local binding is introduced by an
   `export default function NAME` / `export default class NAME` in the same file, bind it
   to `("default", span-of-the-default-declaration)` — then the existing `"default"`
   short-circuit covers `tween`.
2. For `ExportKind::Default` with a named operand, make `name` and `span` agree: either
   record `name` as the operand identifier text (matching what the producer reads at that
   span), or point the span at the `default` keyword. Pick one and use it in both
   `runtime_export` and `declarations_export`.

Must-keep-refusing traps:

- Do not weaken the check to "name mismatch is fine". After the fix the equality must
  still hold exactly; a declaration resolved to a *different* declaration in the same file
  must still refuse.
- `export default <expression>` (not an identifier) has no local declaration name —
  it must stay open/refused, not be given a synthetic name.
- The anonymous-default guard at `type_facts.rs:1848-1854`
  ("canonical default-export target has no declaration identity") must survive.
- `export { x as default }` and `export default x` are different export entries with the
  same public name; keep them distinguishable.

---

### M6 — source attribution by substring, not by materialized root (class a, 1 row)

`solid-recharts@1.0.1` **publishes a `node_modules` directory inside its own archive**:

```
node_modules/solid-recharts/dist/browser/node_modules/csstype/index.d.ts   469670 B  sha256:0eed9868…
node_modules/csstype/index.d.ts                                            894969 B  sha256:ac51dd7d…   (csstype@3.2.3)
```

`verify_snapshot_source_census` (`type_facts.rs:3118-3131`) attributes each producer
source to a snapshot by substring:

```rust
normalized.rsplit_once(marker).map(|(_, relative)| (snapshot, relative))
```

with `marker = "/node_modules/csstype/"`. `rsplit_once` takes the **last** occurrence, so
the vendored file yields `relative = "index.d.ts"` and is charged to the hoisted `csstype`
snapshot. `snapshot.read("index.d.ts")` returns the 894 KB bytes; the producer's digest is
the 469 KB vendored file →
`producer source digest differs from snapshot: index.d.ts`.

Two compounding details:

- The root package's own marker is chained **last**, after every dependency marker
  (`type_facts.rs:3120-3129`), so it loses even though `/node_modules/solid-recharts/` is
  the longer, and correct, marker for this path.
- The `dependency_markers` longest-first sort (line 3102) therefore cannot help.

**Bounded fix.** Attribute a source to the authenticated package root it was
*materialized under*, not to any `/node_modules/<name>/` substring.

- Build the attribution table as `{ materialized_package_root_path -> snapshot }` — the
  private project already knows every one of these (`package_roots`,
  `type_facts.rs:782-806`).
- For a source path, pick the **longest matching path prefix** among those roots,
  including the root plan's own root, all in one comparison; `relative` is the remainder.
- A path matching no authenticated root keeps the current fail-closed behavior
  (`reject_unauthenticated_external_sources`, line 3179).

Under that rule the vendored file resolves to
`solid-recharts` / member `dist/browser/node_modules/csstype/index.d.ts`, which **is** in
that snapshot's `files` map, and the digest matches.

Must-keep-refusing traps:

- A genuine hoisted-vs-nested duplicate of the same package name at two authenticated
  roots must still be distinguished — longest-prefix does that; substring search does not.
  The comment above this loop ("catches a sibling/ancestor installation silently winning
  resolution") is the property to preserve.
- A source under an authenticated root whose remainder is **not** a member of that
  snapshot must still refuse (`producer consulted package source outside the snapshot`).
- Do not fall back to "try the next marker on digest mismatch" — that would let a
  same-named file anywhere in the tree satisfy the census.
- `reject_unauthenticated_external_sources` must be switched to the same prefix rule, or
  the two passes can disagree about who owns a path.

---

### M7 — byte-identical duplicate archive member (class a, 1 row) — **cheap**

`@solid-primitives/start@0.0.4`, integrity verified against
`scripts/ecosystem-benchmark/manifest.json`
(`sha512-qGHsR9ZAyddNDDbx7MbBVfJVw9q+lZlvR63KslNip82+XBYkaq/Hi8rwiMxS/OA9/IAZCHaVP3+awq7S716MUg==`).
Member listing with per-member content digests:

```
  1086 c6d67da3ac65d057 package/LICENSE
  1190 a31ea4af23634a8a package/./dist/index.cjs      <-- duplicate
  1190 a31ea4af23634a8a package/dist/index.cjs        <-- duplicate, identical bytes
  1885 6a7080d15ba36a0a package/dist/index.d.cts
  1104 93250f9f00084061 package/dist/index.js
  5324 5f7e88288ea4e06c package/README.md
  1885 6a7080d15ba36a0a package/dist/index.d.ts
  1590 3205753fb4939749 package/package.json
```

`canonical_member_path` (`contract_certification.rs:2110-2139`) walks
`Path::components()`, which folds the `.` component away, so both entries canonicalize to
`dist/index.cjs`; `contract_certification.rs:1266-1268` then refuses:

```rust
if !seen.insert(package_path.clone()) {
    return Err(ArtifactSnapshotError::DuplicateMember(package_path));
}
```

The duplication is a publish-tool artifact and is benign: the bytes are identical, so any
extractor produces the same file. (`files` is a `BTreeMap` keyed by canonical path, so the
second insert is idempotent and the snapshot root digest is unchanged.)

**Bounded fix.** Refuse only a *conflicting* duplicate:

- on a repeated canonical **file** path, read the member and compare its bytes to the
  already-stored ones; equal → skip, different → keep `DuplicateMember`;
- a repeated **directory** entry with no file of that path is already skipped by the
  `kind.is_dir()` branch — move the `seen` insert so directories do not compete with files;
- respect `expanded_archive_bytes` when reading the second copy (count it, then discard).

Must-keep-refusing traps:

- **Different bytes must still refuse.** Which copy an extractor keeps is tool-dependent;
  that is a genuinely ambiguous artifact.
- Entry *kinds* must match: a file duplicating a directory (or vice versa) still refuses,
  as does anything `validate_topology` rejects.
- The case-fold collision check (`CaseCollision`, line 1269-1276) is a different property
  and must not be relaxed alongside this.
- `UnsafePath` (backslash, NUL, non-`package/` root, `..`) is unaffected and stays fatal;
  the `.` fold must not become a general path-normalization pass.

Upstream also has a (benign) packaging defect here; it does not need to be fixed for the
row to clear, and the checker fix is the right one because the artifact is not ambiguous.

---

### M8 — ECMA-262 export precedence (class a, 1 row) — **cheap, high confidence**

Chain: `motion-solidjs@0.6.0` → peer `motion@12.43.0`
(`dist/es/index.mjs:1: export * from 'framer-motion/dom';`) → `framer-motion@12.43.0`
entrypoint `./dom` → `dist/es/dom.mjs`:

```js
export * from 'motion-dom';
export { delayInSeconds as delay } from 'motion-dom';
export * from 'motion-utils';
```

`motion-dom` exports **both** `delay` and `delayInSeconds`
(`dist/es/index.mjs:152: export { delay, delayInSeconds } from './utils/delay.mjs';`).
`motion-utils` exports no `delay`.

ECMA-262 `ResolveExport` consults a module's local and *indirect* export entries before
its star export entries, so `delay` is unambiguously `motion-dom`'s `delayInSeconds`.
Node agrees:

```
framer-motion/dom delay === motion-dom.delayInSeconds: true
framer-motion/dom delay === motion-dom.delay        : false
delay.length: 2  src head: function delayInSeconds(callback, timeout) { return delay(callback, secondsToMilliseco…
```

`collect_accepted_reexport_candidates` (`rust/crates/solid-facts-backend/src/main.rs:5262`)
iterates `module_level_exports()` in source order and inserts a candidate for the
`ExportKind::All` branch **and** for the named specifier, keyed by
`(AcceptedSemanticIdentity, ExportIdentity)`. Two distinct identities →
`accepted_reexport_summary_for_name` (line 5250-5257) refuses with
`re-exports "delay" from 2 distinct accepted runtime identities`.

Note the Rust snapshot replay already has the right precedence:
`export_bindings.rs::bind_export` checks `description.direct`, then
`description.external_direct`, and only then `description.stars`. The two implementations
disagree; the emission path is the wrong one.

**Bounded fix.** In `collect_accepted_reexport_candidates`, make the walk two-phase per
module:

1. explicit entries first — local declarations, named specifiers (`export { x as name }`,
   `export { x as name } from 'm'`), and the local-import bridge already handled at
   lines 5352-5360. If any explicit entry for `name` exists in this module, resolve
   through it and **do not** consult that module's `export *` targets for `name`.
2. only if no explicit entry exists, walk `ExportKind::All` targets.

Precedence is per-module: an explicit entry in file *F* shadows *F*'s own star exports,
not the star exports of a module *F* itself star-re-exports from.

Must-keep-refusing traps:

- **Two star candidates with distinct identities and no explicit entry stay refused.**
  ECMA-262 makes that name `ambiguous`; importing it is a SyntaxError. Excluding the name
  from the export surface is also acceptable; silently picking one is not.
- Two explicit entries for the same exported name in one module is a duplicate-export
  SyntaxError — still refuse.
- `export * from 'm'` never contributes `default` (already handled, line 5290).
- Type-only specifiers stay excluded.
- Do not "prefer the first candidate in source order" — order is not the rule; entry kind is.

---

### M9 — `IdentityMismatch` is not diagnosable (class d, 2 rows)

Reason (both rows):

```
published graph case-set finalization failed: Type Facts certification failed for published
graph published-graph-case-set: Type Facts certification failed for graph node
@tanstack/solid-query@5.102.5 (.) during live graph export-value verification:
Type Facts live-session identity does not match the certification plan
```

`TypeFactsCertificationError::IdentityMismatch` (`type_facts.rs:4059-4060`) is a **unit
variant raised from 19 sites** across three verification functions. It carries no field,
no expected/actual, and no site. Nothing in the audit sidecar narrows it
(`certification-audit.json` records only the same string).

What can be established without instrumentation:

- **It is specific to these two rows.** Every other tanstack graph row refuses with a
  *family-open* reason instead (`@tanstack/solid-form@2.0.0-alpha.2` →
  `recursive-value-shape … producer's root observation is open`;
  `@tanstack/solid-hotkeys@0.10.0` → `operation-reachability … parameter-rooted read has
  no exact implementation call`). So the failure is not a broken handshake, protocol,
  schema, or build id — those would fail all 418 rows.
- Inside `verify_live_export_value_answer_with_project_census`
  (`type_facts.rs:1604-1662`) only four conditions are package-dependent. The rest
  (`handshake_protocol`, `handshake_schema_sha256`, `handshake_build`, `generation`,
  `project_id`, `demand_sha256`, `snapshot_root`, `demand_graph_root`) are envelope
  identity and would fail uniformly. The four:
  1. `expected_ids != actual_ids` (line 1638) — `expected_ids` is `Vec<String>` sorted;
     `actual_ids` comes from `CertificationInvocationContext::new`, which sorts
     `SourceHash` and rejects duplicates (`invocation.rs:591-599`). An ordering
     divergence between the two sorts would land here.
  2. `answer.transcripts.len() != schedule.export_values.len()` (line 1638).
  3. `transcript.location != scheduled.demand.location` (line 1655).
  4. the implementation-location match (lines 1656-1662):
     ```rust
     match (scheduled.demand.implementation_location.as_ref(), transcript.implementation.as_ref()) {
         (Some(expected), Some(actual)) if expected == &actual.location => {}
         (None, None) => {}
         _ => return Err(TypeFactsCertificationError::IdentityMismatch),
     }
     ```
     This is the strongest candidate: it is the only one whose outcome depends on what the
     producer *chose to return* rather than on a count the checker itself scheduled.
- **The failing graph node is not stable across builds.** The pinned report (release
  binary) names `@tanstack/query-core@5.102.5`; three consecutive debug-binary runs all
  name the root, `@tanstack/solid-query@5.102.5` / `@tanstack/solid-query-persist-client@5.102.5`.
  Same class, different node. The debug reruns are self-consistent.

**Bounded work, in order.**

1. **Make the variant diagnostic.** Replace the unit variant with
   `IdentityMismatch { site: &'static str, field: &'static str, expected: String, actual: String }`
   (or a small enum of the 19 sites). This is mechanical, has no semantic effect, and is
   the precondition for classifying these two rows at all. Every one of the 19 sites today
   produces the same 9-word sentence.
2. Re-run the two probes and read the site.
3. Separately, explain the release-vs-debug node divergence — a verdict that depends on
   the build profile is a defect regardless of which node is "right".

Do **not** relax any of these checks. They are the seam that keeps a live producer answer
bound to the plan it was scheduled for; a wrong relaxation here silently accepts evidence
from a different transaction.

#### M9 implementation result (2026-09-01)

The fielded diagnostic identifies the suspected guard exactly. In both focused
rows the schedule carries `implementation_location=Some(...)` for authenticated
runtime bytes while the producer transcript carries `implementation=None`.
Examples after stable redaction are
`/node_modules/@tanstack/query-core/build/modern/retryer.js:3459-3473 -> None`
and
`/node_modules/@tanstack/query-persist-client-core/build/modern/createPersister.js:6160-6180 -> None`.
No identity check was relaxed and neither row certified.

All former unit-variant sites now name a literal site and field plus oriented
expected/actual values. Path-bearing values are rendered relative to their
`node_modules` coordinate or a redacted private-project marker; unequal raw
identities that redact to the same suffix receive explicit expected/different-
actual annotations. The diagnostic therefore exposes neither a host/user path
nor a per-run temporary directory.

The build-dependent graph-node label came from scheduling the case-set's Type
Facts requests by canonical digest. Request/evidence pairs are now ordered by
package name, version, entrypoint, then digest and re-keyed to the same digest
after acquisition. This changes only which already-failing node is reported
first; it cannot move evidence between nodes. The two focused debug runs
retained their exact-refusal status and selected the same package coordinate.
A release-profile confirmation is deferred to the Round 2 full remeasurement.
Multiple independent open premises can still exist within that selected node;
M9 is now diagnosable, not semantically fixed, and does not choose one failure
as stronger proof.

---

### M10 — optional peer reached through a guarded dynamic import (class a, 1 row)

`@solidjs/testing-library@0.8.10` `package.json`:

```json
"peerDependencies":     { "@solidjs/router": ">=0.9.0", "solid-js": ">=1.0.0" },
"peerDependenciesMeta": { "@solidjs/router": { "optional": true } }
```

`dist/index.js:34-53` — the only reference, a dynamic import inside `try`/`catch` with a
complete fallback:

```js
const routedUi = typeof location === "string" ? lazy(async () => {
    try {
      const { createMemoryHistory, MemoryRouter } = await import("@solidjs/router");
      …
    } catch (e) {
      console.error(`Error attempting to initialize @solidjs/router:\n"${…}"`);
      return { default: () => createComponent(wrappedUi, {}) };
    }
  }) : wrappedUi;
```

`dist/index.d.ts` never mentions `@solidjs/router`. Confirmed:

- `tsc --noEmit` with `allowJs`/`checkJs`/`maxNodeModuleJsDepth: 100` on the probe layout
  reports **nothing** about `@solidjs/router` — so this is not TypeScript's job and the
  absolute rule does not bar the checker from having an opinion.
- At runtime in that layout, `import("@solidjs/router")` rejects with
  `ERR_MODULE_NOT_FOUND` — exactly the case the `catch` exists for.

The checker refuses the whole artifact case at
`packages/cli/scripts/artifact-resolution.mjs:283`:

```js
fail("package-not-found", `${packageName} is not installed above ${importer}`);
```

reached from `findPackageRoot` (line 272-283), which has no notion of an optional edge.

**Bounded fix.** Turn this one refusal into an *edge-level* conditional/inapplicable
result, only when **all** of the following hold:

1. the specifier's package name is listed in the **importing package's**
   `peerDependenciesMeta` with `optional: true`, read from the *authenticated snapshot's*
   `package.json` bytes, not from the installed copy;
2. the module-load site is a **dynamic `import()`** on an unshadowed global `import`
   (the existing scope-resolved Oxc module-load facts already distinguish this — the same
   machinery that distinguishes an unshadowed literal `require`);
3. the package is **genuinely absent** from the authenticated layout — a present but
   unresolvable/broken install still refuses;
4. the emitted artifact case records the absence as part of its identity (conditions /
   layout), so a contract certified without the optional peer is never reused for a layout
   that has it.

Must-keep-refusing traps:

- **A static `import`/`export … from` of an optional peer still refuses.** Node throws
  during module evaluation and no fallback can run; the module is simply broken in that
  layout.
- A dynamic import of a package **not** marked `optional: true` still refuses. Optionality
  is a declaration, not an inference from the import form.
- An optional peer that **is** installed must be resolved and authenticated normally. The
  escape hatch is for absence only.
- The behavior behind the absent edge becomes **uncertifiable**, not "certified as the
  fallback". Here the `location`-routed branch of `render()` must not carry a proven
  claim.
- Never let this generalize to "unresolved ⇒ optional". That is precisely the
  fail-closed boundary the precision contract names.

Note for the phase 21 ledger: `AUTHENTICATED_LAYOUT_REFUSALS`
(`scripts/package-contract-v2-phase21-ledger.mjs:33-39`) groups this row with the four M4
rows under owner `authenticated-dependency-layout`. They are different mechanisms — M4 is
an undeclared peer that TypeScript already errors on, M10 is a *declared-optional* peer
whose absence is designed behavior. The grouping should split.

---

## Acceptance, must-not-clear, controls

### Acceptance (per mechanism)

| id | acceptance |
| --- | --- |
| M2 | `@tanstack/ai-solid@0.19.1\|solid1\|only` certifies. `declarationCandidate` selects `dist/index.d.mts` for `@ag-ui/core`, and the emitted contract's `declarations.path`/`sha256` name that file. |
| M3 | `@tanstack/solid-db@0.2.40\|solid1\|only` reaches a *different* refusal or certifies; the `IR` demand no longer reports a suffix mismatch. |
| M5 | `@solid-primitives/local-store@1.1.4` and `@solid-primitives/tween@1.4.1` both certify. |
| M6 | `solid-recharts@1.0.1\|solid1\|only` certifies; the census attributes `dist/browser/node_modules/csstype/index.d.ts` to the `solid-recharts` snapshot. |
| M7 | `@solid-primitives/start@0.0.4\|solid1\|only` certifies; the snapshot root digest is unchanged from a hypothetical single-member archive. |
| M8 | `motion-solidjs@0.6.0\|solid1\|only` emits a contract for `framer-motion` `./dom` whose `delay` is `motion-dom`'s `delayInSeconds`. |
| M9 | the refusal names a site and a field; only then can these rows be classified. |
| M10 | `@solidjs/testing-library@0.8.10\|solid1\|only` certifies, with the `@solidjs/router` edge recorded as an inapplicable/conditional dependency and the `location`-routed path uncertifiable. |
| M1, M4 | **no change** — these rows must still refuse with the same reason. |

Every fix must land with the pinned-report comparison showing **exactly** the rows above
moving and nothing else. Coverage plus the ownership gate, run against a freshly built
`rust/target/debug/solid-checker-rust` (the bundled-contract / stale-binary trap in
AGENTS.md applies to M6, M7, M8 and M5, which are all Rust).

### Must-not-clear

These must still refuse after every fix above:

1. `@tanstack/solid-start-server` ×3 — `#tanstack-router-entry`. Do not add a fallback
   that resolves an undefined `#` specifier from a *sibling* package's imports map.
2. `@tanstack/solid-query@6.0.0-rc.0` ×2 and `@tanstack/solid-query-persist-client@6.0.0-rc.0` ×2
   — `@solidjs/web`. Do not "helpfully" install or synthesize an undeclared peer.
3. `@solid-primitives/context@0.3.2|solid1|only` — the existing confirmed upstream
   declaration defect. M3 and M6 both touch declaration/source attribution; neither may
   manufacture a nested peer layout.

### Controls (fixture pairs to add with each fix)

| id | positive | negative (must keep refusing) |
| --- | --- | --- |
| M2 | dual-emit package, `exports` with no `types` condition, `.d.mts` and `.d.ts` present → binds `.d.mts` | same package with the `.d.mts` **deleted** → `declarations-not-found`, never falls back to `.d.ts` |
| M3 | `import * as N from './n.js'; export { N }` where `n.d.ts` is a snapshot member → accepted | the same namespace file replaced by a copy **outside** the snapshot (hoisted sibling) → refused |
| M5 | `export default createX;` and `export default function createX(){}; export { createX }` → both certify | `export default (a) => a` (anonymous) → still refused as having no declaration identity |
| M6 | package archive containing `dist/x/node_modules/<dep>/index.d.ts` with bytes ≠ the hoisted `<dep>` → attributed to the owning archive | two authenticated snapshots of one name at different roots, producer reads the nested one → still distinguished, digest mismatch still refuses |
| M7 | tarball with `package/./a.js` and `package/a.js`, identical bytes → accepted | same paths with **different** bytes → `duplicate archive member` |
| M8 | `export * from 'm'; export { y as x } from 'm';` where `m` exports both `x` and `y` → resolves to `y` | `export * from 'm1'; export * from 'm2';` where both export `x` with distinct identities → still refused |
| M10 | optional peer absent + guarded dynamic `import()` → edge inapplicable, case certifies | (a) same package, **static** `import` of the optional peer → refused; (b) same dynamic import but the peer is **not** `optional: true` → refused; (c) optional peer **present** → resolved and authenticated normally, no escape hatch taken |

Fixture stubs for these must be byte-faithful to the published manifests for every field a
proof depends on — in particular `exports` condition order, the absence of a `types`
condition (M2), and `peerDependenciesMeta` (M10). A stub that adds a `types` condition
`@ag-ui/core` does not have would make M2 untestable.
