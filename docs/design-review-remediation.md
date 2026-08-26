# Design-review remediation plan

Source: full-catalog design review of main @ fdd9c045 (2026-08-12), three passes —
Solid 1.x catalog (41 rules), Solid 2.0 catalog (37 rules), and repo
composability. Verdict: 29/38 v1 rules and 28/34 v2 rules clean; the failures
are peripheral (docs, severity policy, coverage, dispatch typing), not core
logic. The architecture's seams hold in the actual dependency graph; the gap
to "lego we assemble" is assembly ergonomics, not structure.

Every item below must land under the standard gates
(`SOLID_TYPEFACTS_BIN=$PWD/bin/solid-typefacts`; rebuild `bin/` first):
`cargo +1.97 test --workspace`, `bun scripts/coverage.mjs`,
`bun scripts/parity.mjs`. House bar: every commit individually green.
Known trap: the dialect seam (vocabulary + engine + both catalogs) moves as
one piece when `CallbackSemantics` or the defect/violation projection
changes; new fixtures need their `node_modules/solid-js/package.json`
gitignore exception line.

---

## Phase 1 — Trust fixes (small diffs, highest payoff)

The diagnostic contract was codified in d5180459; these are the places the
code or docs contradict it.

### 1.1 Reconcile SC9011 with the severity contract
`docs/rules/README.md:49` says "Uncertifiable findings always carry error
severity", yet `reactive-source-uncaptured` ships uncertifiable findings at
warning in both catalogs (`rust/dialects/solid-v2/rules/src/rules.rs:117`,
v1 equivalent at `rust/dialects/solid-v1/rules/src/rules.rs:165`). Found
independently by both catalog reviews. Decide one way:
- (a) scope the README sentence — SC9011 is advisory-by-design because it has
  no proven-violation form; or
- (b) escalate SC9011 findings the way the owner rules do
  (`Finding::for_owner_requirement`).
Recommendation: (a); the rule is deliberately a nudge, not a defect.

### 1.2 `v1/no-direct-mutation` severity vs its own message
Manifest severity is warning, but every finding is a proven dropped write —
the message says "the write is dropped and nothing re-runs"
(`rust/crates/solid-reactive-ir/src/findings.rs:375`), the same certainty
class as SC2001's error. Upstream's `reactivity` was warn, which explains the
tier but is nowhere written down. Either raise to error (declare the
divergence in `fixtures/upstream-parity/deviations.json`) or document the
upstream-mirroring rationale next to the SC1004 note in
`docs/rules/README.md`.

### 1.3 SC3005 `settled-cleanup-unowned` placement and severity
It is emitted from the same `missing_owners` table as the `no-owner-*`
family (`rust/dialects/solid-v2/rules/src/lib.rs:237`), describes the same
consequence, yet wears an SC3xxx code, breaks the family naming pattern, and
is an error while `no-owner-cleanup` (SC4002) is a warning — with no
documented rationale on either page. Codes are cheapest to move now, before
the beta catalog hardens: either re-code into the SC4xxx family with a
`no-owner-*` name, or write the differential rationale on both doc pages the
way SC1004's is written.

### 1.4 Wasm wire types — the one place documentation lies
- `packages/wasm/index.d.ts:4` publishes
  `compilerOptions?: { jsx?: boolean; typescript?: boolean }`, a shape the
  Rust struct rejects (`deny_unknown_fields`,
  `rust/crates/solid-facts/src/compiler.rs:16`). Mirror the real
  `CompilerOptions` fields or drop the field from the d.ts.
- Same file: "TypeFacts v2 closure" comment → v3
  (`typefacts::v3::EntityDemand`, `docs/typefacts-protocol-v3.md`).
- Extend `packages/wasm/test/smoke.test.mjs` to send one non-empty `sources`
  entry with `compilerOptions` exactly as the d.ts spells them, so the two
  can never drift silently again.

## Phase 2 — Documentation debt

### 2.1 Rewrite the ten v1 stub pages
`docs/rules/v1/{imports, jsx-no-duplicate-props, jsx-no-script-url,
no-array-handlers, no-proxy-apis, no-react-deps, no-react-specific-props,
prefer-for, prefer-show, jsx-uses-vars}.md` are one-sentence stubs in
implementation vocabulary; every finding routes users to them. Fix the two
inaccurate ones first:
- `imports.md` says "from Oxc import facts"; the rule validates against the
  dialect's generated export index
  (`rust/crates/solid-reactive-ir/src/upstream_compat/solid1x_imports.rs:1`).
- `no-array-handlers.md` says "legacy array-form"; the tuple form is a
  supported, type-unsafe shorthand (`solid1x_attributes.rs:731`).
Then bring the rest to the standard of the good pages (mechanism,
consequence, example, fix). Consider strengthening the docs test
(`rust/dialects/solid-v1/rules/src/rules.rs:283`) beyond file existence —
e.g. minimum length or a required example fence.

### 2.2 Document the SC7003/SC9003 dual-name convention user-side
The one-code-two-names design is explained only in a source comment
(`rust/dialects/solid-v2/rules/src/rules.rs:159`). Add the explanation to all
four doc pages, including the consequence that suppressing by code silences
both surfaces. Minor: `invalid-refresh-target` also fires on arity errors
(`static_api.rs:138`) where "target" is not the defect — note or rename.

### 2.3 Write down the severity principle
Next to the existing SC1004 note in `docs/rules/README.md`: "enabled rules
mirror upstream 0.14.5 base policy exactly; the four upstream-off rules are
enabled at &lt;severity&gt; because &lt;reason&gt;" — or change their tiers in 2.5.

### 2.4 SC2001 store-setter wording
Message uses the RC diagnostic name `REACTIVE_WRITE_IN_OWNED_SCOPE`
(`rust/dialects/solid-v2/rules/src/lib.rs:71`) while provenance admits store
setters, and the hint offers `ownedWrite` only via `createSignal` though the
implementation honors it on any source-creating call
(`source_discovery.rs:304`). Word by provenance.

### 2.5 Decide the upstream-off enablement question (design decision)
Upstream ships `no-array-handlers`, `no-proxy-apis`, `prefer-classlist`,
`prefer-show` **off**; this catalog has no off tier, so they became
error/error/warn/warn. Practical sting: `no-proxy-apis` errors on every
`solid-js/store` import (`solid1x_structure.rs:127`), and only the ESLint
adapter can disable a rule (`packages/cli/eslint.cjs:274`) — CLI/daemon/LSP
have no channel; `.solid-checker/rule-options.json` is options-only and
fail-closed. Options:
- (a) add an enablement/severity tier to the manifest (off-by-default for
  these four), or
- (b) extend `rule-options.json` to accept per-rule enablement.
Either way, record the principle (2.3). This is the only Phase-2 item that
is a real design decision rather than debt.

## Phase 3 — Coverage gaps

### 3.1 v1 wording paths with zero fixtures
`v1/no-owner-boundary` (`solid-v1/rules/src/lib.rs:161`),
`v1/primitive-in-directive-application` (`lib.rs:132`), and
`v1/package-contract-missing` never fire under the v1 dialect in any test —
the boundary/directive/contract fixtures all run v2. Same move as commit
b80d383b's `no-owner-v1`: add v1-dialect fixtures (remember the gitignore
exception per fixture), or extend the catalog-prose program
(`solid-facts-backend/src/dialect.rs:376`) with a boundary requirement and a
directive creation.

### 3.2 SC9004 `execution-map-incomplete`
Zero end-to-end coverage in either dialect; only synthetic exercise. Either
build a fixture that genuinely produces an incomplete execution map, or
document in the rule page that it is unreachable with current compiler facts
and covered synthetically by design.

### 3.3 Broaden the async corpus (SC5001/SC5002)
The 2.0 catalog's headline rules each fire exactly once
(`fixtures/findings-snapshots/reactive-ir__async-boundary.json`). Add
pending reads via `latest`/`isPending`, refetch-in-flight, and
nested-boundary patterns. Also thin: SC1005/SC1006/SC1007 on the v2 side,
SC7004, invalid-affects-target, both SC9003 names (one finding each).

## Phase 4 — The projection-seed refactor (the one substantial piece)

Two independent concern classes share one root cause, so fix them together.

**Symptoms:**
- Stringly-typed IR→catalog dispatch with silent catch-alls:
  `write.setter.starts_with("refresh(")` (`solid-v2/rules/src/lib.rs:60`,
  paired with `format!("refresh({name})")` at `static_api.rs:240`);
  `match operation.primitive.as_str()` with `_ =>` fallthrough (`lib.rs:106`);
  `match requirement.operation.as_str()` routing unknown operations to
  `NoOwnerEffect` with the wrong prose (`lib.rs:226`). The static-violation
  path already panics on unmapped identities (`findings.rs:229`) — the
  stronger contract covers one of four channels.
- Verbatim-duplicated selection predicates between the two `solve_measured`
  functions (`solid-v1/rules/src/lib.rs:33,51` ≡
  `solid-v2/rules/src/lib.rs:35,53`), plus ~60% copy-identical
  `static_defect_finding` arms; only fixtures catch drift.

**Shape of the fix:** move selection predicates and finding scaffolding into
`solid-reactive-ir` as typed finding *seeds* (enum kind + subject + evidence
locations), exhaustively matched — `project(program, &impl CatalogWording)
-> Vec<Finding>`. Each catalog becomes a wording table (seed kind → rule
identity, message, hint) plus a declared capability set for the tables it
projects (v1 skips actions/async/cleanup-return; the differentials stay
explicit). A future catalog is then literally a table.

**En route:** give `ReactiveWrite` a typed refresh discriminant instead of
the `refresh(` prefix; consider a dedicated rule code for
refresh-in-owned-scope (SC2002 set the split precedent) for suppression
granularity.

**Sequencing caution:** this moves the dialect seam — vocabulary, engine, and
both catalogs land as one piece per the house rule. Fixture snapshots update
in the same commit as the code that moves them.

### 4.b Thread evidence through `StaticViolation`
Today every static violation gets "the invalid API shape is statically
present at this call" (`findings.rs:236`) — wrong for stylistic rules
(`self-closing-comp`, `prefer-show`, `prefer-for`, `imports`,
`event-handlers` rename advice). Add an evidence field to `StaticViolation`
(or let the catalog resolve evidence per rule). Natural to do inside the
Phase-4 seam move; standalone otherwise.

### 4.c SC9005 wording into the catalogs
`package-contract-missing` is the single rule whose prose lives outside its
catalog — composed in `solid-facts-backend/src/diagnostics.rs:350`, shared
verbatim, contradicting the v1 crate's "every sentence" claim
(`solid-v1/rules/src/lib.rs:5`). Fold the wording into each catalog's table
during Phase 4. Related: `solid1x_options.rs` hardcodes catalog namespace
strings ("v1/no-innerhtml") as config keys inside the IR crate — inverted
seam direction; re-key or move ownership.

## Phase 5 — Seam repairs (restore the conventions to literally true)

### 5.1 De-spell `structural_accessor_spans`
Shared backend code hardcodes `import.module.starts_with("solid-js")` and a
primitive-name list (`rust/crates/solid-facts-backend/src/lib.rs:1131,1172`)
— the exact "shared code must not switch on API spelling" breach, failing
silently for a future dialect. It already runs under a `&'static Dialect`;
ask the vocabulary instead.

### 5.2 Drop the dialect re-export
`pub use solid_v2_compiler::NativeCompilerFacts`
(`solid-facts-backend/src/lib.rs:70`) names a dialect crate outside the
registry; `tests/sessions_process.rs:427` should obtain the compiler via the
bundle's factory.

### 5.3 Two small deepenings
- Move `RuleOptions` (1.x-only, apologetic comment at
  `solid-reactive-ir/src/lib.rs:25`) out of the engine root or rename to its
  scope.
- Split `solid-facts-backend/src/lib.rs` (1,945 lines): wire types
  (`SourceFile`, `SourceChange`, `TypeFactsProvider`) apart from
  orchestration.

## Phase 6 — Assembly ergonomics (the "lego" payoff)

### 6.1 `docs/adding-a-dialect.md` — the forward checklist
Adding a dialect today touches ~12 places; the only guidance is the
retrospective in `rust/ARCHITECTURE.md:93`. Enumerate: workspace member +
pinned compiler dep; two crates under `rust/dialects/<id>/`; `Version` enum +
vocabulary impl + exports index + contract JSON in `solid-dialect`;
`dialect::ALL` + `by_version`; bundled contract in
`diagnostics.rs:658` + `pkg/contracts/bundled/`; `Makefile:70` stanzas;
`scripts/check-bundled-contracts.mjs:20`; `packages/cli/lib/rules-<id>.json`
+ the `manifests` map and `dialect === "v1"` special case in
`packages/cli/eslint.cjs:310,335`; detection fixture +
`scripts/coverage.mjs:78,94` lists; `packages/wasm/index.d.ts:13`; README +
ARCHITECTURE tables; `structural_accessor_spans` (until 5.1 lands).

### 6.2 Shrink the checklist mechanically
Each hand-keyed list is a place a new dialect silently doesn't exist:
- key `check-bundled-contracts.mjs` and the Makefile contract stanzas off one
  per-dialect manifest;
- generate `eslint.cjs`'s `manifests` map from the shipped `rules-*.json` by
  enumeration;
- replace the `dialect === "v1"` special case with a `namespace` field in the
  manifest JSON.

### 6.3 Unify the id namespaces
One axis, three-plus namespaces: dialect ids (`solid-v1`/`solid-v2`),
contract-gen ids (`solid-js-1x`/`solid-js`/`solidjs-web`,
`Makefile:71`), eslint manifest keys (`v1`/`v2`), and near-twin filenames for
*different* contract sets (`pkg/contracts/bundled/solid-js-v1.json` vs
`rust/crates/solid-dialect/contracts/solid-js-1x.json` — runtime-embedded vs
vocabulary-test/exports-index). Pick one scheme; add/extend a README in each
contracts directory naming which set is which and which script regenerates
it. Zero behavior change.

### 6.4 (Optional) per-dialect cargo features
`dialect-v1`/`dialect-v2` (default = both) on `solid-facts-backend` and
`solid-checker-wasm`, gating each `dialect::ALL` entry and its compiler
dependency. Motivation: a v2-only wasm saves roughly one JSX compiler
(~600 KB of ~1.25 MB combined compiler weight; the catalogs themselves are
~20 KB each) for payload-sensitive targets, and a build with one feature off
mechanically detects seam violations (it would catch 5.2 today). Skip if no
payload-sensitive consumer exists yet.

---

## Suggested order and dependencies

| Order | Items | Size | Notes |
| --- | --- | --- | --- |
| 1 | 1.1–1.4 | S | Independent one-commit fixes |
| 2 | 2.1–2.4 | S–M | Docs only; 2.1's two inaccurate pages first |
| 3 | 2.5 | M | Needs a decision; unblocks severity wording in 2.3 |
| 4 | 3.1–3.3 | M | Fixtures; watch the gitignore-exception trap |
| 5 | 4 + 4.b + 4.c | L | One seam-move series; snapshots move with code |
| 6 | 5.1–5.3 | S–M | 5.1/5.2 easier after 4; independent otherwise |
| 7 | 6.1–6.3 | M | 6.1 first (documents reality), then shrink it |
| 8 | 6.4 | M | Optional; after 5.2 or as its enforcement |

Explicitly *not* to change (reviewed and found deliberate): the wording
duplication between catalogs; byte-faithful upstream heuristics
(`docs/precision-backlog.md`); bundled multi-check ported rules
(name-preserving parity); the SC1004 severity differential; the
createReaction and async-tracked-scope dialect differentials pinned by the
`dialect-solid-1x`/`dialect-solid-2` fixture pair; the prose-guard, drift,
and runtime-severity-mirror tests.
