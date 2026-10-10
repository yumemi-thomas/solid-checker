# Compiler binding fixes and installed output evidence

## Result

The declaration failures in the previous compiler experiment are fixed. With
the same old compiler facts, exact component bindings increase from 84 to 103
and expression contexts from 101 to 139. All 32 previously excluded package
source runs enter a separate implementation program. The rejected application
`For` resolves to the overload selected by TypeScript. Two newly exposed package
`Show` uses retain the exact declared overload family without selecting one.

A cached newer fact compiler adds one matching source/mode: native `Select` in
DOM mode. The final result is 104 component bindings and 141 expression contexts
in the 54 matching-output runs. The other 42 compiler outputs remain open.

A second experiment uses the application's actual RC.9 compiler, without a
fact fork. It finds 380 unchanged expressions copied into generated getters,
including 320 on runs where the fact fork's output differs. These are narrow
source links, with lexical binding, tracking, owner and runtime invocation open.
They do not replace the compiler execution model or establish new defects.

All artifacts retain `authority:false` and `certification:false`. Production
compiler pins, analyzer behavior and package bytes remain unchanged.

## Fixes

### Exact overload and package source resolution

`compiler-jsx-resolution-v1.mjs` follows the exact TypeScript symbol and aliases.
A checked application overload is admitted only when the resolved signature's
declaration belongs to that symbol's declaration set. It does not choose the
first or shortest declaration. The actual published `solid-js@2.0.0-rc.9` `For`
has three overloads; TypeScript selects the declaration for the existing valid
application use.

`compiler-package-bindings-v3.mjs` keeps the two configured application programs
and their authenticated clean typing runs. A third child analyzes the 22 actual
published package source paths with `allowJs:true` and `checkJs:false`. This
program locates declarations; it makes no clean package-typing claim. Each child
has a 2 GiB V8 heap cap and a 60-second timeout. Every one of the 96 source/mode
decisions remains accounted for, including empty operation runs and refusals.

For unchecked package source, a declared function overload family can be located
as a set. Each member must be a bodyless function declaration in the same actual
declaration file. The selected overload remains null. Merged function/namespace
symbols, unresolved names and a signature from another symbol remain open.

| Measure | Previous | Fixed with old producer | Fixed with newer cached producer |
| --- | ---: | ---: | ---: |
| Exact output matches | 53 | 53 | 54 |
| Component bindings | 84 | 103 | 104 |
| Exact expression contexts | 101 | 139 | 141 |
| Runs outside the source program | 32 | 0 | 0 |
| Unresolved component declarations | 1 | 0 | 0 |
| Output drifts | 43 | 43 | 42 |

The final 104 bindings cover 64 distinct source uses: 56 application declaration
bindings, 42 Kobalte, four Solid and two web-runtime bindings. DOM and SSR uses
count separately in the binding total. These counts measure evidence recovered,
not defects found or package-use accuracy.

### Compiler compatibility

Standalone `compiler-facts-probe-v2/` uses the already cached semantic-only fork
distribution `16f0988e0313c9ce1a06ed479799b45262eaca43`, based on upstream
`8cfa2724f893e07b03e977bd2902c6d0e6019ca7` and implementation
`c58f531a7707529273a23f343a087036e493f12d`. Its adapter speaks the existing
normalized fact seam. No compiler lowering or fork source is edited. This
revision is an older cached compiler, not the application's RC.9 release.

All 96 DOM/SSR fact runs remain output neutral with tracing enabled/disabled.
They contain the same 1,069 source and 1,090 generated operations as before.
Generated operation enumeration remains partial. Universal and dynamic modes
remain refused. The production compiler identity remains unchanged.

`compiler-output-differences-v1.mjs` characterizes the 42 remaining output drifts.
Nineteen have the same compared syntax after treating a static computed getter
key and its literal getter key as the same review representation. Twenty-three
have other syntax differences, including runtime helper imports and generated
markup. The comparison omits trivia and positions. It authorizes zero bindings;
every drift still fails exact emitted-byte admission. A proper RC.9 fact-only
rebase remains necessary for full compiler-context attachment on these sources.

## Further experiment: use the installed compiler directly

`compiler-installed-getters-v1.mjs` requests source maps from the actual retained
`@solidjs/compiler@2.0.0-rc.9` native artifact. All 96 mapped compilations preserve
the exact code from the earlier unmapped compilations. Maps contain the exact
original source bytes. Among 483 generated getters with one return expression,
405 starts map to an exact, unique original JSX expression. None also has an
exact expression end mapping. A full-range source-map requirement therefore
recovers zero facts.

Successor V2 tests a different, narrower claim: a source expression was copied
into a generated getter. `compiler-expression-copy-v1.mjs` requires:

- an exact generated start mapping to one original JSX expression;
- identical expression text and parsed syntax structure;
- exact source mappings for every identifier and literal in that expression;
- a getter body containing exactly one return statement.

There is no nearest-point interpolation or search by matching expression text.
Repeated expression text is disambiguated by its exact source position. End
mapping is recorded separately and stays unobserved for all accepted copies.

The final result is 380 copied expressions with 990 mapped identifiers/literals.
There are 103 open getters: 78 lack an exact unique source start, and 25 have a
changed expression copy. Of the 380 witnesses, 320 occur in runs where the older
fact fork's output differs from RC.9. Those whole-output refusals remain intact.

The claim is deliberately limited to generated syntax and source association.
A copied expression does not establish its lexical binding after lowering,
whether anyone reads the getter, what tracking or owner exists then, whether a
package invokes a callback, or whether the application is wrong. Generated
helper callbacks and a complete execution census remain outside this experiment.

This gives a practical route for reducing fork maintenance: derive narrow
positive facts from the installed compiler's exact output and maps, while using
compiler-owned facts for timing, ownership and lowering behavior that output
provenance cannot establish. Both layers need exact package/source identity;
neither requires a handwritten behavior contract for every package.

## Verification and artifacts

- Eleven resolution checks pass against actual published RC.9 declarations.
  The valid corpus passes the real TypeScript CLI. Aliases, namespace imports,
  a shadowed `For`, member declarations and UTF-8 spans are covered. Unresolved
  names, foreign signature declarations and merged namespaces remain refused.
  The deliberately unresolved negative has TS2304; no checker diagnostic is
  emitted for it.
- Nine expression-copy checks pass, including seven refusals for changed text,
  nearby or missing mappings, duplicate points, a wrong source index and a wrong
  descendant mapping. The positive case has repeated source expression text.
- Fourteen standalone adapter tests pass. Offline build, formatting and Clippy
  pass. The initial formatting check found a copied trailing blank line; it was
  removed before the final probe build and fact collection.
- Source/binding audits reconcile both the unchanged old-producer comparison
  and the final newer-producer run. Eight modified evidence reports are refused.
- The installed-output audit repeats all 96 actual native compilations and
  checks exact code/map identity, input bytes and the complete getter partition.
  Copy reconstruction shares the separately tested verifier; it is not an
  independent proof of that verifier's semantics.
- `make verify-fast`, schema JSON validation, dialect manifest validation and
  `git diff --check` pass. New modules pass syntax checks; previous sealed input
  bytes are preserved. The previous 823 runtime tests are unchanged and are not
  rerun for this compiler/source slice.

Final generated artifacts live under ignored `rust/target/`:

- `compiler-source-feedback-v3/results.json` — final 96-run fact comparison;
- `compiler-package-bindings-v3-{old-fixed,final}.json` — isolated fix comparison
  and final binding result;
- `compiler-source-fixed-old-final-audit-v1.json` and
  `compiler-source-fixed-final-audit-v1.json` — reconciled source evidence;
- `compiler-jsx-resolution-tests-v2/results.json` — valid typing and resolution
  checks; V1 preserves the failed harness attempt before its JSX runtime path
  was corrected to the package's published export;
- `compiler-source-negative-audit-final-v2/results.json` — eight evidence refusals;
- `compiler-output-differences-final-v1.json` — drift characterization;
- `compiler-installed-getters-v1/results.json` and `compiler-installed-getters-v2/results.json`
  — strict endpoint trial and narrower copy trial;
- `compiler-expression-copy-tests-v1.json` and `compiler-installed-getter-audit-v1.json`
  — copy regressions and repeated actual native compilation;
- the compiler probe build/test/Clippy logs, workspace verification log,
  combined summary and successor handoff freeze.

Full `make verify`, production process/coverage/ownership/contract gates and
release checks are deferred for this research-only slice. No production Rust,
snapshots, accepted contracts, public schemas or dialect manifests change.

## Remaining work

The 42 compiler drifts, 103 unmatched emitted getters, partial generated
enumeration, runtime-provider identity, callback timing, tracking/ownership,
result relevance, causal diagnosis and safe repair remain open. Earlier noisy
raw async controls remain unresolved; this slice changes source context evidence,
not the runtime warning policy. General package coverage and real-application
warning precision are not established by these counts.
