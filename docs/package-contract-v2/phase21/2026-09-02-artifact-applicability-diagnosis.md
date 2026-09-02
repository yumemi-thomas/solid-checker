# Artifact applicability for the never-attempted partial proposals

Read-only diagnosis. Repo `/Users/thomas/Documents/Github/solid-checker`, branch
`codex/phase19a-authenticated-proof-policy`, HEAD `84ea72e1` (verified). Debug
checker built by `make build-checker-debug`; `bin/solid-typefacts` reused as
found (stamp already at source
`6ae37483b7740fb7a0b21c8c160159b11102aa56c088c900074bbe98f40a5b3d`, build id
`dev`).

Every claim below was checked against the **installed `package.json` and the
installed file bytes** of a `--keep-temp` reproduction, not against
documentation. The 11-probe unpatched reproduction (`base.json`) is
bit-identical to `benchmarks/ecosystem/report.json` on every row: same class,
same refused/inapplicable counts, same reasons.

Scratch: `<scratch>/applic/` — `base.json`/`base.md` (unpatched reproduction),
`probe-resolve.mjs` (a direct driver for `prepareArtifact` /
`selectPackageExportTarget` / `artifactCaseDisposition`), `ceiling.json` /
`ceiling.md` (§3), `main.rs.orig` + `generate-package-contract.mjs.orig`
(pre-patch copies used to confirm the revert).

---

## 0. Headline

The 9 target rows are **three unrelated mechanisms**, not one, and only one of
them is a defect.

| Mechanism | Rows | Owner | Verdict |
|---|---|---|---|
| **M1. `.d.ts` runtime target vs. the batch source lookup** | `@solidjs/h`, `@solidjs/image`, `@solidjs/universal`, `@kobalte/solidbase` (56 cases) | Rust `main.rs`, **a straight defect** | the refusal message is *false*; the file **is** in the configured project |
| **M2. non-emitting entrypoint (type-only module / ambient `.d.ts`)** | `@kobalte/core` (11), `@kobalte/utils` (1), and M1's rows after M1 is fixed | Rust emitter, correct *as a refusal*, wrong *as a row class* | a sound narrow applicability rule exists |
| **M3. side-effect-only / empty / CJS entrypoint** | `@kobalte/core` (41 test files), `@solidjs/diagnostics` `./vitest`, `@solid-devtools/{ext-adapter,babel-plugin,shared}` | Rust emitter | **no sound rule separates the kobalte test files from the ext-adapter control. They stay refused.** |

Consequences that decide the build:

* `@solidjs/diagnostics@2.0.0-rc.3`'s `./vitest` is **not** an enumeration
  artifact. It is `expect.extend({...})` — the exact ext-adapter shape — and it
  is a **correct refusal**. The 09-01 scoping study tagged it
  "different-owner: artifact-case enumeration"; that tag is wrong.
* `@kobalte/core@0.13.13` cannot be recovered by any applicability rule: 41 of
  its 52 refusals are M3.
* `@solid-devtools/shared@0.20.0` is a **third** shape — a genuinely *empty*
  module (`dist/index.js` is 0 bytes, `src/index.ts` is literally `export {}`) —
  and it stays not-attempted regardless, because its `./chunk-DTKGRNV6` case
  refuses for an unrelated reason (`no declaration target exists`).
* `@solid-primitives/utils@6.4.1` **is attempted and does certify** (see §4).
  The rootless story's blocker is narrower than "utils is unaccepted".

---

## 1. Per case: why it is an entrypoint, what it is, which axis, which code path

### 1.1 `@kobalte/core@0.13.13|solid1|only` — 52 cases, all M2/M3

Installed `package.json` (`.../node_modules/@kobalte/core/package.json`), verbatim:

```json
"files": ["dist", "src", "NOTICE.txt"],
"exports": {
  ".":        { "types": "./dist/index.d.ts", "solid": "./dist/index.jsx", "default": "./dist/index.js" },
  "./*":      { "types": "./dist/*/index.d.ts", "solid": "./dist/*/index.jsx", "default": "./dist/*/index.js" },
  "./src/*":  "./src/*"
}
```

**Why these files are entrypoints.** `"./src/*": "./src/*"` is an
**unconditional bare-string wildcard over the published source tree**
(`files` ships `src`). The finite wildcard census walks the artifact's members
and expands the pattern over each one, so all 491 files under `src/` become
artifact-case candidates. The recorded resolution trace names the branch
exactly:

```
"trace": { "branch": "/exports/.~1src~1*",
           "steps": [ {"condition":"subpath","target":"./src/badge/badge.test.tsx"},
                      {"condition":"target","target":"./src/badge/badge.test.tsx"} ] },
"conditions": []
```

**The `solid` condition is not involved.** The brief's hypothesis ("does kobalte
export `./src/*` under the `solid` condition so the compiler can read JSX
source") is **false for 0.13.13**: `./src/*` is a bare string with no condition
object at all, and the `solid` condition appears only on `.` and `./*`, pointing
at `dist/*.jsx`. Both the runtime axis and the declaration axis select the same
`src` file (verified: `selectPackageExportTarget` with `axis:"runtime"` and
`axis:"declaration"` return the identical path).

**What the 52 files are** (`find` + byte inspection):

| n | shape | example | evidence |
|---|---|---|---|
| 41 | Vitest test module, **zero exports** | `src/badge/badge.test.tsx` | no `export` token anywhere in the file; body is `describe(...)`/`it(...)` with ambient globals |
| 11 | **type-only** module | `src/tabs/types.ts` is one line: `export type TabsActivationMode = "automatic" \| "manual";` | every module-level statement is a type declaration |

All 41 test files are refused; 11 of the 12 `src/**/types.ts` are refused. The
twelfth, `src/selection/types.ts`, **certifies** — because it contains
`export class Selection extends Set<string>`, a runtime value. That is already a
proof-based distinction (an export census, not a suffix), and it is the single
most useful existing datum here.

**Axis.** Runtime. The declaration axis is satisfied (the same file).

**Refusing code path.** Rust, not JS:
`rust/crates/solid-facts-backend/src/main.rs:5280-5285`

```rust
if exports.is_empty() {
    return Err(format!(
        "emit package contract: entry file {} has no runtime ESM exports",
        entry_file.display()
    ).into());
}
```

`exports` is built in `contract_exports_for_entry_file`
(`main.rs:5187-5286`) from `exported_names_for_file` (`main.rs:6841`), which
walks `AstFacts::module_level_exports()` and drops `type_only` specifiers
(`main.rs:6867`, `:6903`, `export_is_type_only` at `:6910`), then intersects
with the declaration-axis census (`main.rs:5205-5212`). JS relays it through
`stableRefusalReason` at
`packages/cli/scripts/generate-package-contract.mjs:1226` (hence the
`solid-checker-rust:` prefix).

**The JS side already knows.** `prepareArtifact` records
`resolution.exports === {}` for `badge.test.tsx` and for `tabs/types.ts`, and
non-empty for `selection/types.ts` — `exactExportBindings`
(`packages/cli/scripts/artifact-resolution.mjs:1447-1493`) computes the
runtime∩declaration name intersection before Rust ever runs. So the refusal is
reachable pre-analysis; **but the empty-surface predicate is exactly the one the
2026-09-02 revert rejected**, because ext-adapter's `dist/index.js` produces the
same `{}`.

**Undeclared test dependencies** (the brief's frontier hypothesis). Verified:
`vitest`, `@solidjs/testing-library` and `@testing-library/user-event` appear in
**no** dependency field of `@kobalte/core`'s manifest
(`dependencies` = `@floating-ui/dom`, `@internationalized/number`,
`@kobalte/utils`, `@solid-primitives/props`, `@solid-primitives/resize-observer`,
`solid-presence`, `solid-prevent-scroll`; `devDependencies` = `@kobalte/tests`,
`esbuild-plugin-solid`, `tsup`; `peerDependencies` = `solid-js`), and none of
the four is installed. The closure records them, but **as the same generic
hazard as `solid-js`**:

```
{"kind":"unaccepted-external-dependency","source":"./src/badge/badge.test.tsx:@solidjs/testing-library", "affectedDomains":[all nine]}
{"kind":"unaccepted-external-dependency","source":"./src/badge/badge-root.tsx:solid-js", "affectedDomains":[all nine]}
```

There is **no** unresolved-vs-external distinction on the wire today. §2(b)
explains why building one would not be sound anyway.

### 1.2 `@kobalte/utils@2.0.0-alpha.0|solid2|only` — 1 case, pure M2

```json
"files": ["dist", "src"],
"exports": {
  ".":       { "types": "./dist/index.d.ts", "solid": "./dist/index.js",
               "import": {"types":"./dist/index.d.ts","default":"./dist/index.js"},
               "require": "./dist/index.cjs" },
  "./src/*": "./src/*"
}
```

Same unconditional `./src/*` wildcard, 7 source members, 6 certify. The refused
one, `src/types.ts`, is verbatim:

```ts
export type ValidationState = "valid" | "invalid";
export type Orientation = "horizontal" | "vertical";
export interface RangeValue<T> { start: T; end: T; }
```

Two type aliases and one interface: the module emits **no** JavaScript at all.
Same code path as §1.1 (`main.rs:5282`). **This row is the one clean win: it has
exactly one refusal and it is M2.**

### 1.3 `@solidjs/diagnostics@2.0.0-rc.3|solid2|only` — 1 case, M3 (correct refusal)

```json
"./vitest": { "types": "./dist/vitest.d.ts", "default": "./dist/vitest.js" }
```

An **explicitly declared** subpath — no wildcard, no custom condition. Its
runtime target `dist/vitest.js` is a real ESM module whose entire body is:

```js
import { expect } from "vitest";
import { DiagnosticsAssertionError, expectDiagnostic, ... } from "./assertions.js";
import { assertBudget } from "./budgets.js";
function runAssertion(check, negatedMessage) { ... }
expect.extend({ toHaveNoDiagnostics(...) {...}, ... });
```

and whose declaration sibling `dist/vitest.d.ts` ends in `export {};` after a
`declare module "vitest" { interface Matchers ... }` augmentation. `vitest` is
declared as an **optional peer dependency**
(`"peerDependencies": {"vitest": ">=2.0.0"}`,
`"peerDependenciesMeta": {"vitest": {"optional": true}}`) and is not installed.

This is a **side-effect-only matcher-registration module**: import-time effects,
zero exports, by design. It is the same shape as
`@solid-devtools/ext-adapter`'s `dist/index.js` and must keep the same
disposition. Same code path (`main.rs:5282`).

The row's other case, `./package.json`, is already `non-module-target`
inapplicable — the 08-31 rule working as intended.

### 1.4 `@solidjs/h@2.0.0-rc.3|solid2|only` — 2 cases, M1

```json
"files": ["dist","types","types-cjs","package.json","jsx-runtime/dist", ...],
"exports": {
  ".": { "import": {"types":"./types/index.d.ts","default":"./dist/h.js"},
         "require": {"types":"./types-cjs/index.d.cts","default":"./dist/h.cjs"} },
  "./jsx-runtime": { ... }, "./jsx-dev-runtime": { ... },
  "./types/*": "./types/*"
}
```

`"./types/*": "./types/*"` — an unconditional wildcard over the published
**declarations** directory, which holds exactly `hyperscript.d.ts` and
`index.d.ts`. So both `.d.ts` files become **runtime**-axis artifact cases.
`types/index.d.ts` is one line
(`export { default, type HyperScript, type HyperElement } from "./hyperscript.js";`)
and `types/hyperscript.d.ts` is a pure ambient declaration file
(`declare const _default: HyperScript; export default _default;`).

Recorded refusals (conditions `[]` for both):

```
./types/hyperscript.d.ts  contract emission batch target 3 names source outside its configured project: <package-root>/types/hyperscript.d.ts
./types/index.d.ts        contract emission batch target 4 names source outside its configured project: <package-root>/types/hyperscript.d.ts
```

### 1.5 `@solidjs/image@0.1.0|solid1|only` — 1 case, M1

```json
"files": ["dist","env.d.ts"],
"exports": { ".": "./dist/index.jsx", "./env": "./env.d.ts",
             "./vite": "./dist/vite.js", "./package.json": "./package.json",
             "./style.css": "./dist/style.css" }
```

`./env` is an **explicit, non-wildcard, unconditional** export whose target
is a `.d.ts`. Its whole content is two `declare module "image:*"` /
`declare module "*?image"` ambient blocks — a Vite `env.d.ts` shim. Refusal:
`contract emission batch target 1 names source outside its configured project:
<package-root>/env.d.ts`.

Note that this row already carries the 08-31 rule correctly:
`./package.json` and `./style.css` are `non-module-target` inapplicable.
This row has **exactly one refusal**, and it is M1.

### 1.6 `@solidjs/universal@2.0.0-rc.3|solid2|only` — 2 cases, M1 (+ latent M2/over-proof)

`"./types/*": "./types/*"` again; `types/` holds `index.d.ts`
(`export * from "./universal.js";`) and `universal.d.ts`.

`universal.d.ts` is the **important** one:

```ts
export interface RendererOptions<NodeType> { ... }
export interface Renderer<NodeType> { ... }
export declare function createRenderer<NodeType>(options: RendererOptions<NodeType>): Renderer<NodeType>;
```

`export declare function createRenderer` is a *value* name. So this case is
**not** type-only by export census: `moduleDescription`
(`artifact-resolution.mjs:1256-1263`) hits `ts.isFunctionDeclaration` with an
export modifier on **both** axes and records `createRenderer` as a runtime
binding. Importing `@solidjs/universal/types/universal.d.ts` at runtime yields
nothing at all, so anything the emitter says about `createRenderer` *for this
artifact case* would be a false claim. **This is the over-proof trap of M1:
"fix the scoping refusal" without an applicability premise turns a false refusal
into a false certification.** (§3 measures exactly that.)

### 1.7 `@kobalte/solidbase@0.6.13|solid1|only` — mechanism only, will not certify

`"./default-theme/*": {"solid":"./dist/default-theme/*","import":"./dist/default-theme/*","types":"./dist/default-theme/*"}`
— a wildcard over a mixed directory. The census enumerates 28 `.d.ts` members
× 2 active condition arms (`solid`, `import`) = **56 M1 refusals**, alongside
94 `non-module-target` inapplicabilities (the 08-31 rule already absorbing the
`.map`/`.json`/`.css` members). The row also carries two refusals nothing here
touches:

```
./client        accepted dependency virtual:solidbase/components has no exact runtime binding for export mdxComponents
./config/route  resolved target <package-root>/src/config/route-config.js is not a file   (files ships "/src", the target does not exist)
```

so it stays not-attempted under every rule below.

### 1.8 The controls, byte-verified

| Row | Target | Bytes | Class |
|---|---|---|---|
| `@solid-devtools/ext-adapter@0.17.0` | `exports: null`, `module: dist/index.js` (legacy field), `type: "module"` | real ESM: 5 `import` statements, then `startListeningWindowMessages(); postWindowMessage("ResetPanel"); ... createInternalRoot(() => {...})`. **Zero exports.** `dist/index.d.ts` is **1 byte**. | M3 — side-effect-only, correct refusal |
| `@solid-devtools/babel-plugin@0.3.1` | `exports: null`, `main: dist/index.js`, **no `"type"` field** | `"use strict"; var __create = Object.create; ... 0 && (module.exports = { devtoolsPlugin });` — a **CommonJS** bundle | M3 / `UnsupportedModuleSystem`, correct refusal |
| `@solid-devtools/shared@0.20.0` | `"." → {"@solid-devtools/source":"./src/index.ts","types":"./dist/index.d.ts","default":"./dist/index.js"}`, plus `"./*"` | `dist/index.js` is **0 bytes**; `src/index.ts` is `export {}`; `dist/index.d.ts` is `export {};` | a **third** shape: an *empty* module. Row also refuses `./chunk-DTKGRNV6` with `no declaration target exists for <package-root>/dist/chunk-DTKGRNV6.d.ts`, so it stays partial-success regardless |
| `@solid-primitives/{controlled-props@1.0.0-next.3, virtual@1.0.0-next.4}` | `dist/index.jsx` absent from the artifact | publisher defect, untouched here |
| `@solidjs/vite-plugin@3.0.0-next.34` | `./virtual-solid-manifest`, `./boundary-modules` select no active condition | untouched here |

### 1.9 M1: the exact defect

The message is **false**. `types/hyperscript.d.ts` *is* in the tsconfig the
generator wrote.

1. `analyzeArtifactsBatch`
   (`packages/cli/scripts/generate-package-contract.mjs:841-869`) writes one
   batch tsconfig whose `files` is the **union** of `projectFiles(...)` across
   the batch, and gives each target `sourceFiles: projectFiles(candidate.prepared.resolution)`
   (`:863`). `projectFiles` (`:493-500`) = the runtime path plus every
   `runtime`/`literal-dynamic-chunk` closure entry. For
   `./types/index.d.ts` that set is `{types/index.d.ts, types/hyperscript.d.ts}`.
2. Rust fills `request.sources` from the producer
   (`rust/crates/solid-facts-backend/src/main.rs:2333`):
   `request.sources = typescript.configured_sources()?;`
3. `configured_sources` → `Operation::Sources`
   (`rust/crates/typefacts/src/session.rs:1078`) →
   `session.go:343 LifecycleSources` → `closure.SourceFiles(ctx)` →
   **`apps/solid-typefacts/internal/typefacts/tsgo/project.go:387-397`**:

   ```go
   programFiles := p.program.SourceFiles()
   for _, sourceFile := range programFiles {
       if sourceFile.IsDeclarationFile {
           continue
       }
       ...
   ```

   **Every declaration file is dropped.** This is correct for its ordinary
   caller (Rust wants the files it will build facts *for*), and it silently
   breaks the batch path.
4. `main.rs:2363-2371` indexes those sources by canonical path, and
   `contract_emission_target_sources` (`main.rs:335-362`) requires **every**
   target source to be present, erroring at `:346-353`.

So the predicate is "this target's project files include a `.d.ts`", and the
message names a scoping condition that does not hold. Two further notes:

* Only the **batch** path has this failure. `analyzeArtifact`
  (`:741-819`) writes the same tsconfig but passes `--emit-contract`, never a
  per-target source list, so there is nothing to look up. The batch path is the
  primary path for every case (`:1155-1164`); the singleton path is only the
  ordered fallback for non-primary members of an identity group.
* The refusal is charged to the **wrong target**: `@solidjs/h`'s
  `./types/index.d.ts` (target 4) is refused naming
  `types/hyperscript.d.ts`, a file belonging to its closure.

---

## 2. The authenticated applicability premise, per class

The three constraints from the 2026-09-02 revert
(`docs/precision-backlog.md:10936-10943`) are binding: **no suffix guessing, no
empty-surface rule, no pre-authentication `.d.ts` classification**. Together with
the 08-31 doctrine (a target real consumers reach and fail on stays a refusal),
they eliminate most of what looks available.

### 2(a) M2 — "the entrypoint's authenticated bytes emit no JavaScript"

**Premise (narrowest sound form).** An artifact case is
`inapplicable: non-emitting-module-target` when, for the runtime target selected
by the authenticated replay:

1. the selected path is a **regular-file member of the authenticated archive**
   (`ArtifactSnapshot::from_archive`,
   `rust/crates/solid-facts-backend/src/contract_certification.rs:1277-1293`
   already refuses every non-`is_file` member as `UnsupportedMember`, refuses
   case collisions, and refuses duplicate members with differing bytes — so
   symlink/hardlink aliasing is closed *before* the premise is asked), and
2. those exact bytes parse as a TypeScript module **every one of whose
   module-level statements is non-emitting**: a type alias, an interface, an
   `enum`-free `declare`, an `import type` / `export type`, or an `export {}`
   with no value specifier — i.e. the emitted JavaScript is provably the empty
   module, and
3. the module has **no import with a value clause and no top-level expression
   statement** (this is what makes it "emits nothing", not "exports nothing").

**Where it lives.** In **Rust**, on the certification side, keyed to
`resolve_snapshot_export`
(`contract_certification.rs:1633-1679`) — the only place that reads bytes it has
authenticated. The generator side (`packages/cli/scripts/*.mjs`) reads the
*installed tree*, which is pre-authentication; that is precisely the "pre-replay
declaration guess" the revert rejected. This is the `ArtifactApplicability`
result phase 20 slice 4 reserved
(`docs/package-contract-v2/phase20/2026-08-30-row-verification-unblock-plan.md:703-716`),
with the existing `ARTIFACT_APPLICABILITY.TypeOnlyExport`
(`"verifier-proved-type-only"`,
`generate-package-contract.mjs:36`) as its name.

**Cost note that decides the design.** The row *class* is produced by
generation, before certification runs. So a verifier-owned premise cannot by
itself change a row's class: the generator must emit the case as a *proposal
case carrying a declared applicability claim* which certification then proves or
refuses. That is the honest shape and it is not small. A generator-side
shortcut — classify from the installed bytes and omit the case — is
what was reverted; it would be sound only if the generator's read of the file
were itself bound to the archive digest, which today it is not.

**Must-not-clear traps, with concrete fixtures.**

| Trap | Fixture | Must stay |
|---|---|---|
| side-effect-only ESM with zero exports | `ext-adapter`-shaped: `import {a} from "dep"; a();` and no export | **refused** (has a value import and a top-level call → fails premise 3). Pins the exact 09-02 regression. |
| matcher-registration module | `diagnostics`-shaped: `import {expect} from "vitest"; expect.extend({...})` | **refused** (same) |
| CJS bundle | `babel-plugin`-shaped: `"use strict"; ... module.exports = {...}` | **refused** (top-level statements) |
| empty module | `shared`-shaped: `export {}` and a 0-byte sibling | this one *does* satisfy the premise. Decide deliberately: an empty module genuinely has nothing to certify, but "0 bytes" is also what a **broken build** looks like. Recommend **refused**, by requiring premise 2's statement list to be **non-empty** — a module that declares at least one type is a deliberate type module; a file with no statements at all is indistinguishable from a publish defect. |
| `.d.ts` naming a value export | `universal/types/universal.d.ts`-shaped: `export declare function f(): void;` | **inapplicable** under this premise (an ambient declaration emits nothing) — and this is the *only* rule that stops §1.6's over-proof. Must **not** be answered by the export census, which sees `f` as a runtime name. |
| `.d.ts` renamed to `.js` | a `.js` member whose bytes are `export declare function f(): void;` | must get the **same** answer as the `.d.ts` — proving the rule is byte-based, not suffix-based |
| `.tsx` under the `solid` condition | `{"solid": "./src/widget.tsx"}` where `widget.tsx` exports a real component | **certified**, unchanged. This is why the rule must be about *emission*, not about "TypeScript source is not a Node module": `vite-plugin-solid` compiles that file, and kobalte's own `.` → `dist/index.jsx` depends on it. |
| a target with a `.map`/`.css` suffix | `wildcard-asset-entrypoints` (existing) | still `non-module-target`; the two classes must not overlap |

**Yield of 2(a) alone: 4 rows** — `@kobalte/utils@2.0.0-alpha.0`, `@solidjs/h`,
`@solidjs/image`, `@solidjs/universal` — measured in §3. `@kobalte/core` still
holds 41 M3 refusals and stays not-attempted.

**Important consequence, measured in §3.2: 2(a) does not need M1 fixed.** The
premise is decided at the **disposition** stage, before the case is prepared or
sent to the emitter, so the case is omitted from the batch entirely and
`main.rs:346` is never reached. M1 stays a latent false-message defect for
*emitting* targets whose closure includes a declaration file (§6.4), but it is
not on the path to these four rows. That is what makes this a single small
slice instead of a Rust emitter change.

### 2(b) M3 — no sound rule; the kobalte test files stay refused

Structurally, in authenticated bytes, `src/badge/badge.test.tsx` and
`ext-adapter/dist/index.js` are the **same object**: a module with value
imports, top-level expression statements, and an empty export census. Three
candidate separators, all rejected:

1. **"imports an undeclared dependency."** `@solidjs/testing-library` is in no
   dependency field of kobalte's manifest, and `vitest` is declared *optional
   peer* in `@solidjs/diagnostics`. So the predicate "the specifier is named in
   no dependency field" does separate them — but it is **not a proof of
   inapplicability**. A consumer that also installs `vitest` and
   `@solidjs/testing-library` (every kobalte contributor does) resolves
   `@kobalte/core/src/badge/badge.test.tsx` and executes it. Under the 08-31
   doctrine that is "real consumers can reach it and we could not prove it" —
   a **refusal**, with a better `applicability` tag
   (`unresolved-dependency-frontier`) and no change to the row class. It buys
   nothing.
2. **"the entrypoint came from a wildcard we expanded."** Tempting — the author
   wrote one pattern, our census wrote 508 entrypoints — but a
   wildcard-expanded subpath is genuinely importable, and half the *certified*
   kobalte surface comes from the same wildcard. Using provenance-of-the-census
   as a semantic premise would also silently reclassify every `./*` package in
   the corpus.
3. **"empty export surface."** This is the reverted rule verbatim.

**Conclusion: M3 stays refused, and `@kobalte/core@0.13.13` stays
not-attempted.** That removes 41 of the 52 kobalte-core cases and the largest
single number in the 09-01 census from the recoverable set.

### 2(c) M1 — what the refusal means and what the fix is

"Names source outside its configured project" means: *this batch target's
`sourceFiles` list contains a path the Type Facts producer did not report in
`Sources`* — and the producer never reports a declaration file
(`project.go:390`). It is **not** a tsconfig `include` problem: the generator
writes `files:` explicitly and the file is in it.

Two candidate fixes:

* **F1 (minimal, wrong on its own).** Let
  `contract_emission_target_sources` tolerate a target source the producer
  legitimately omits, so the declaration file participates as a program input
  via the tsconfig only. This is what §3's diagnostic patch does. It removes the
  false refusal — and **immediately exposes the §1.6 over-proof**: with no
  applicability premise, `@solidjs/universal/types/universal.d.ts` proceeds to
  summarize `createRenderer` from an ambient declaration. F1 must never ship
  without 2(a).
* **F2 (correct).** Emit the declaration-file case on the **declarations axis
  only**, and record the runtime axis as `inapplicable:
  non-emitting-module-target` per 2(a). Concretely: an export-map target whose
  authenticated bytes are an ambient declaration file has no runtime module, so
  the runtime axis has nothing to select. Note the seam already half-exists —
  `RUNTIME_EXTENSIONS` (`artifact-resolution.mjs:32`) deliberately excludes
  `.d.ts`, but that list only drives *extensionless candidate suffixing*, so an
  **explicit** `.d.ts` target passes through the runtime axis unchecked
  (`selectPackageExportTarget` returns it; verified).

**Must-not-clear traps for M1.**

| Trap | Must stay |
|---|---|
| a target absent from the archive | `MissingPublishedTarget` refusal (`themes@0.0.1-next.0`, `controlled-props`, `virtual`) — F1 must not turn "not in the project" into "skip it" for a file that simply does not exist. The diagnostic patch in §3 does exactly that and is therefore **unshippable as written**. |
| a target source the producer omits for a *different* reason (unparsable, over-limit) | refusal, not skip. F1's `continue` cannot distinguish these; only F2 (decide the axis from the member bytes) can. |
| `@solidjs/universal/types/universal.d.ts` | must **not** certify `createRenderer` |

---

## 3. Ceiling by diagnostic skip

Two temporary patches (both reverted, see §5):

* `rust/crates/solid-facts-backend/src/main.rs:346` — under
  `SOLID_CHECKER_DIAG_SKIP_UNSCOPED_SOURCE`, `continue` past a target source
  absent from `sources_by_path` instead of erroring (bypasses M1).
* `packages/cli/scripts/generate-package-contract.mjs:1219` — under
  `SOLID_CHECKER_DIAG_INAPPLICABLE_NO_ESM`, route any refusal whose reason
  contains `has no runtime ESM exports` into the `inapplicable` array instead of
  `refusals` (bypasses M2 *and* M3, so this is an upper bound, not a proposal).

Two runs were needed, because the first one exposed that M1 has **two** layers.

### 3.1 Run A — bypass M1's batch lookup + reclassify every no-ESM refusal

`ceiling.json`, 9 probes, 419 s wall for the slowest.

| Row | class | refused | inapplicable | certification |
|---|---|---|---|---|
| `@kobalte/core@0.13.13` | **success** | 0 | 52 | **refused** — `witness-acquisition`, family `callable-path` |
| `@kobalte/utils@2.0.0-alpha.0` | **success** | 0 | 1 | **certified** (6.3 s) |
| `@solidjs/diagnostics@2.0.0-rc.3` | **success** | 0 | 2 | **certified** (5.8 s) |
| `@solidjs/h@2.0.0-rc.3` | partial-success | **2** | 0 | not attempted |
| `@solidjs/image@0.1.0` | partial-success | **1** | 2 | not attempted |
| `@solidjs/universal@2.0.0-rc.3` | partial-success | **2** | 0 | not attempted |
| `@solid-devtools/ext-adapter@0.17.0` | `all-cases-inapplicable` | 0 | 1 | not attempted |
| `@solid-devtools/babel-plugin@0.3.1` | `all-cases-inapplicable` | 0 | 1 | not attempted |
| `@solid-devtools/shared@0.20.0` | partial-success | 1 | 5 | not attempted |

Findings from run A:

* **`@kobalte/core@0.13.13` is worth nothing.** With all 52 cases waved through
  it reaches certification and is **refused**:

  ```
  policy-2 case-set finalization failed: Type Facts certification failed during live
  export-value verification: Type Facts demand
  sha256:a8312af52943a29cbc9ada627d74bf8e435bd13ebc4de1c71cd6cce8350187ea is locally open:
  callable-path (artifact-case:e8ff883a09f41fde84ed7c7d54ca1348524957a5bdcf5e89b387bb88d013d878:Accordion):
  export root is not compiler-proved callable or constructable
  ```

  Cost: `demandPlanning` 168 s + `witnessAcquisition` 144 s = **313 s of
  certification**, on top of 106 s of generation, for a row that ends refused.
  Since 41 of its 52 cases are M3 and stay refused anyway (§2b), the honest
  yield is zero and this is the correct place for the first-blocker caveat.
* **`@solidjs/diagnostics` certified — but only through the leak.** The run-A
  patch cleared `./vitest`, which §1.3/§2b show must stay refused. Under a
  sound rule this row stays `partial-success` with 1 refusal, and its
  `dependencyPlan` is `{complete: null, roots: 0}` (verified in `base.json`),
  so the runner's second gate disjunct cannot rescue it. **Sound yield: 0.**
* **The controls hold structurally.** `ext-adapter` and `babel-plugin` land in
  the existing `all-cases-inapplicable` class
  (`scripts/ecosystem-benchmark/lib/classify.mjs:57`, `:125`), which is **not**
  `success`, so even the crude bypass never certifies them. `shared` keeps its
  `./chunk-DTKGRNV6` refusal. That is a useful safety property of the runner,
  but it is *not* a substitute for the rule being sound — a wrong
  `inapplicable` still silently deletes a real refusal from the ledger.
* **M1 is not one guard, it is two.** Bypassing `main.rs:346` moved h / image /
  universal to the *next* fail-closed guard, `main.rs:5234-5241`:

  ```
  emit package contract: entry file <package-root>/types/hyperscript.d.ts is not part of the TypeScript project
  emit package contract: entry file <package-root>/env.d.ts is not part of the TypeScript project
  emit package contract: entry file <package-root>/types/universal.d.ts is not part of the TypeScript project
  ```

  `files_by_canonical_path` is built from the fact program's sources, which
  exclude declaration files for the same producer reason. **So the pipeline
  structurally cannot build facts for a `.d.ts` entry file at all**, and there
  is no "fix M1 and certify it" path. F1 is dead; only F2 (§2c) exists. This
  also retires §1.6's over-proof worry at this level: the emitter never reaches
  `createRenderer`.

### 3.2 Run B — the actually-proposed disposition (`.d.ts` runtime target → inapplicable)

`ceiling-dts.json`, patch in `artifactCaseDisposition`
(`generate-package-contract.mjs:101`) only, no Rust patch.

| Row | class | refused | inapplicable | certification |
|---|---|---|---|---|
| `@solidjs/h@2.0.0-rc.3` | **success** | 0 | 2 | **certified** (3.95 s) |
| `@solidjs/image@0.1.0` | **success** | 0 | 3 | **certified** (3.53 s) |
| `@solidjs/universal@2.0.0-rc.3` | **success** | 0 | 2 | **certified** (3.05 s) |

**All three certify, in under 4 s each.**

### 3.3 The ceiling

| Row | Class | Ceiling |
|---|---|---|
| `@kobalte/utils@2.0.0-alpha.0\|solid2\|only` | M2 (type-only module) | **verified** |
| `@solidjs/h@2.0.0-rc.3\|solid2\|only` | M2 (ambient `.d.ts`) | **verified** |
| `@solidjs/image@0.1.0\|solid1\|only` | M2 (ambient `.d.ts`) | **verified** |
| `@solidjs/universal@2.0.0-rc.3\|solid2\|only` | M2 (ambient `.d.ts`) | **verified** |
| `@solidjs/diagnostics@2.0.0-rc.3\|solid2\|only` | M3 | certifies only through an unsound clear — **0** |
| `@kobalte/core@0.13.13\|solid1\|only` | M3 (41) + M2 (11) | refuses at `callable-path` even fully waved through — **0** |
| `@kobalte/solidbase@0.6.13\|solid1\|only` | M1/M2 (56) + independent | **0** (`virtual:solidbase/components`, missing `route-config.js`) |

**Ceiling = 4 rows**, 344 → 348 verified out of 418, and the 25 not-attempted
rows become 21. The four all cost under 7 s of certification each.

---

## 4. `@solid-primitives/utils@6.4.1|solid1|only` — actual row status

**It is attempted, and it certifies.** From both `report.json` and the
reproduction:

```
class:                     partial-success
refusedArtifactCases:      1
inapplicableArtifactCases: 2     (both unpublished-conditional-target, "@solid-primitives/source")
certificationAttempt:      { attempted: true, status: "certified", stage: "catalog-publication",
                             owner: "configured-issuer", ordinaryAnalysis: { receiptAuthenticated: true,
                             exactCaseSelected: true } }
dependencyPlan:            { complete: true, status: "exact-leaf-refusal", roots: 1 }
```

It reaches the runner gate (`scripts/ecosystem-benchmark/run.mjs:1176-1182`)
through the **second** disjunct — a complete dependency plan with one root — not
through `class === "success"`. So `partial-success` is not by itself
disqualifying.

**But the receipt does not cover the entrypoint rootless needs.** The published
catalog holds exactly one contract:

```
objects/7372abac….main.json   import.specifier = "@solid-primitives/utils/immutable"
                              import.requestedEntrypoint = "./immutable"
                              34 exports (add, clamp, concat, …)
```

and the one refused case is the **root**:

```json
{"entrypoint":".","conditions":[],"stage":"artifact-case","applicability":"runtime-module",
 "reason":"accepted dependency solid-js/web has no exact runtime binding for export isServer"}
```

Cause, from the installed bytes: `dist/index.js` line 2-8 is
`import { isServer } from "solid-js/web"; ... export { isServer }; export const isClient = !isServer;`
— a re-export of a dependency binding. `acceptedExternalBinding`
(`artifact-resolution.mjs:1325`) looks up
`acceptedDependencies["solid-js/web"].exports.isServer.runtime`, finds nothing,
and `artifact-resolution.mjs:1381-1388` refuses. The dependency plan's own
leaves confirm the chain is broken upstream:
`csstype` and `solid-js/types/reactive/signal.js` are both `target-not-found`.

`@solid-primitives/rootless@1.5.4`'s `dist/index.js:3` imports the **root**
(`import { asArray, access, noop, createMicrotask, trueFn } from "@solid-primitives/utils"`),
i.e. exactly the case that never certifies.

**So the 09-02 composition diagnosis's attribution is half right.** Two
independent gaps, and naming only one of them understates the work:

1. **The accepted lane is not wired at all.** `run.mjs:1607-1630` passes
   `--catalog`, `--issuer-configuration`, `--trust-configuration-output`,
   `--audit-output`, `--proposal-refusal-audit`, `--proposal`, `--entrypoint`,
   and **never** `--accepted-contracts`. Each probe gets a fresh
   `outputDir`/catalog (`run.mjs:1539-1542`), so no receipt from one row is ever
   visible to another. Every row re-plans its dependencies privately through
   `mergeProposalDependencies`
   (`packages/cli/scripts/certify-contract.mjs:1035-1084`), which is the
   unauthenticated proposal lane.
2. **Even with (1) wired, utils 6.4.1's root receipt does not exist** — it is
   blocked on `solid-js/web`'s `isServer`, which is a *bundled-contract /
   dependency-chain* question, not a composition-plumbing one.

The rootless receipt would therefore still be vacuous after wiring the accepted
lane. The reachable first step is smaller than "composition": give
`solid-js/web`'s `isServer` an exact binding.

---

## 5. Revert

All three diagnostic patches are reverted from source, the debug checker was
rebuilt from the reverted source (the patched env-var string is gone:
`strings rust/target/debug/solid-checker-rust | grep -c SOLID_CHECKER_DIAG_SKIP_UNSCOPED_SOURCE`
→ `0`), and the controls were re-measured against that build:

```
$ git diff --stat
                       (empty)
$ git status --short
?? packages/cli/solid-reactivity.json.refusals.json      (pre-existing, untouched)
$ git diff --exit-code -- rust/crates/solid-facts-backend/src/main.rs \
                          packages/cli/scripts/generate-package-contract.mjs
                       both identical to HEAD
```

Control re-measure on the reverted build (`controls.json`) reproduces the
baseline exactly:

| Row | class | refused | attempt |
|---|---|---|---|
| `@kobalte/utils@2.0.0-alpha.0` | partial-success | 1 | not attempted |
| `@solidjs/h@2.0.0-rc.3` | partial-success | 2 | not attempted |
| `@solidjs/universal@2.0.0-rc.3` | partial-success | 2 | not attempted |
| `@solid-devtools/ext-adapter@0.17.0` | `no-exported-surface` | 1 | not attempted |

No `benchmarks/ecosystem/report-probes-*.md` was created; every report went to
`<scratch>/applic/`. No tracked file changed. No commit.

---

## 6. Yield estimate, first-blocker caveat, and recommendation

### 6.1 Yield

**4 rows, one rule.** The measured ceiling is `@kobalte/utils@2.0.0-alpha.0`,
`@solidjs/h@2.0.0-rc.3`, `@solidjs/image@0.1.0`, `@solidjs/universal@2.0.0-rc.3`
— 344 → **348** verified of 418, 25 → **21** not attempted, at under 7 s of
certification per row.

The four are one class, not two, once the premise is stated correctly. A
type-only `.ts` module and an ambient `.d.ts` are the same object under the
premise **"every module-level statement in the authenticated bytes is
non-emitting"**:

| Row | member | statements | emits |
|---|---|---|---|
| `@kobalte/utils` `./src/types.ts` | `src/types.ts` | 2 type aliases + 1 interface | nothing |
| `@solidjs/h` `./types/index.d.ts` | `types/index.d.ts` | 1 export declaration (`default` + 2 `type`) | nothing |
| `@solidjs/h` `./types/hyperscript.d.ts` | `types/hyperscript.d.ts` | `type`, `declare const`, `export type`, `export default` of a `declare const` | nothing |
| `@solidjs/image` `./env` | `env.d.ts` | 2 `declare module` blocks | nothing |
| `@solidjs/universal` `./types/index.d.ts` | `types/index.d.ts` | `export * from "./universal.js"` in an ambient file | nothing |
| `@solidjs/universal` `./types/universal.d.ts` | `types/universal.d.ts` | 2 interfaces + `export declare function` | nothing |

Stating it as emission rather than as a suffix is what makes it survive the
09-02 lesson **and** what makes it answer `universal.d.ts`, which the export
census gets wrong (§1.6).

### 6.2 First-blocker caveat

Every number above is a *first-blocker* number.

* The four rows were measured with a diagnostic patch at the **generation**
  stage only; the sound implementation moves the premise behind the
  authenticated replay (§2a), and that replay can find a case the generator's
  pre-authentication read got wrong. Any such case becomes a refusal and the
  row falls back to `partial-success`. The 4 is therefore an upper bound.
* `@kobalte/core@0.13.13` is the cautionary sample: it *passed* the artifact
  stage completely and then refused at a `callable-path` demand 313 s later. No
  applicability work predicts the next stage's answer.
* `@solidjs/diagnostics`, `@kobalte/solidbase` and `@kobalte/core` each have at
  least one blocker no applicability rule can touch, so they are **0**, not
  "0 for now".
* Nothing here touches the ecosystem wall-time budget favourably: the four rows
  add ~14 s of certification. Waving `@kobalte/core` through would add ~313 s
  for a refusal, which is another reason not to.

### 6.3 Recommendation: build it, as one slice

4 rows for one narrow, byte-based rule is worth it — mostly because the rule
also fixes a **false refusal message** that would otherwise stay in the ledger
(§1.9: "names source outside its configured project" is not true of any of these
files) and closes a latent over-proof (§1.6). But build only the M2 rule.
**Do not** build M3, and **do not** build F1.

Slice contents:

1. **JS generator** — `packages/cli/scripts/artifact-resolution.mjs`: add a
   `nonEmittingModuleTarget(path)` predicate beside `nonModuleTargetExtension`
   (`:139-147`), implemented over the already-parsed `ts.SourceFile` that
   `moduleDescription` obtains (`:1113`), answering "every module-level
   statement is non-emitting **and** the statement list is non-empty".
   Non-emitting = type alias, interface, `declare` of anything other than a
   non-`declare` `enum`, `declare module`/`declare global`, `import type` /
   `export type`, an export declaration all of whose specifiers are type-only,
   and an `export {}` with no specifiers. Emitting = any value import clause,
   any expression statement, any non-`declare` function/class/`enum`/variable.
   Then wire it into `artifactCaseDisposition`
   (`packages/cli/scripts/generate-package-contract.mjs:81-119`) as a third
   class, and add it to `ARTIFACT_DISPOSITION` (`:49-52`) — name it
   `non-emitting-module-target`, and reuse the existing
   `ARTIFACT_APPLICABILITY.TypeOnlyExport` spelling
   (`"verifier-proved-type-only"`) for the applicability tag.
2. **Rust emitter** — nothing changes for the four rows (the case is omitted
   from the proposal exactly as a refused case is, so `resolve_snapshot_export`
   and the case-set completeness check in `main.rs` stay in agreement, per the
   08-31 entry's "there is no Rust twin to add"). But the **verifier-owned**
   premise is what makes it sound, and it is the part that must land in this
   slice, not later: prove the same predicate in
   `contract_certification.rs` against `snapshot.read(path)` for every case the
   proposal declared inapplicable, and refuse the whole proposal if the
   generator's claim disagrees. Without that, this is the reverted
   pre-authentication classification again with a better predicate.
   `ArtifactSnapshot::from_archive` (`contract_certification.rs:1277-1293`)
   already closes the archive member-kind / symlink / hardlink / case-collision
   invariants the 09-02 revert named, so the premise sits on top of them rather
   than beside them.
3. **Sidecar class** — add `non-emitting-module-target` to the
   `inapplicable` array's vocabulary. No `refusalVersion` bump (the 08-31
   entry's reasoning holds: the array is additive and no consumer counts it).
4. **Fixtures** (`fixtures/package-contracts/`, modeled on
   `wildcard-asset-entrypoints`, whose README is the template):
   * `non-emitting-module-target` — a package with `"./types/*": "./types/*"`
     shipping an ambient `.d.ts` that names a *value* export
     (`export declare function f(): void`), a type-only `.ts`, and a real
     module sibling. The `.d.ts` and the type-only module are inapplicable, the
     sibling certifies, zero refusals. Pins `@solidjs/{h,universal}` and
     `@kobalte/utils`.
   * `non-emitting-module-target-control` — four must-not-clear cases in one
     package: (a) an `ext-adapter`-shaped side-effect-only ESM
     (`import {a} from "./dep.js"; a();`, no export) → **refused**; (b) a
     `diagnostics`-shaped `import {expect} from "vitest"; expect.extend({})`
     with `vitest` an optional peer → **refused**; (c) a CJS bundle with
     `module.exports` → **refused**; (d) a **`.js`** member whose bytes are
     `export declare function f(): void;` → **inapplicable**, proving the rule
     reads bytes and not the suffix.
   * extend `wildcard-asset-entrypoints` with an empty (0-byte) member →
     **refused**, pinning the "statement list must be non-empty" guard so a
     broken build is never silently cleared. (`@solid-devtools/shared`'s
     `dist/index.js` is exactly this.)
   * a `declaration-sibling-reach` regression check that the rule never fires
     on a *declaration axis* target.
5. **Ledger** — the `phase19-audit` cut pin does not count fixtures
   (`scripts/package-contract-phase19.mjs` pins receipts, policy markers, and
   forbidden shortcuts, not the corpus size), but `fixtures/package-contracts/`
   is an `ACTIVE_JSON_PREFIXES` entry in `scripts/package-contract-phase18.mjs:44`,
   so every new fixture JSON must carry the audited envelope. Add the new
   fixture names to `fixtures/package-contracts/corpus.json`.
6. **Runner gate untouched.** `scripts/ecosystem-benchmark/run.mjs:1176-1182`
   stays exactly as it is: the rows arrive through `class === "success"` on
   their own, which is the whole point.

### 6.4 What stays open after the slice

* `@kobalte/core@0.13.13` — 41 M3 refusals; and even waved through it refuses
  at `callable-path` demand `sha256:a8312af5…` for `Accordion`
  ("export root is not compiler-proved callable or constructable").
* `@solidjs/diagnostics@2.0.0-rc.3` `./vitest`, `@solid-devtools/ext-adapter@0.17.0`,
  `@solid-devtools/babel-plugin@0.3.1` — M3, no sound separator exists;
  deliberately fail-closed.
* `@solid-devtools/shared@0.20.0` — the empty-module question is answered
  *refused* by choice, and `./chunk-DTKGRNV6` refuses independently
  (`no declaration target exists`).
* `@kobalte/solidbase@0.6.13` — 56 cases would move, the row will not:
  `virtual:solidbase/components` has no runtime binding for `mdxComponents`,
  and `src/config/route-config.js` is not a file.
* **M1's false message survives for any *emitting* target the producer omits.**
  The slice makes the four rows never reach `main.rs:346`, but it does not
  repair the message or the two-layer guard (`main.rs:346`, `main.rs:5234`).
  Any future package whose runtime closure legitimately includes a declaration
  file will hit the same false refusal. Worth a separate, small
  message-and-attribution fix (the refusal is charged to the wrong target
  index, §1.9).
* `@solid-primitives/utils@6.4.1`'s **root** case, and therefore every
  consumer's composition against it: blocked on `solid-js/web`'s `isServer`
  binding, plus the accepted lane not being wired in the runner at all (§4).
  Neither is an applicability question.
