# unaudited-solid-release

`SC9014` · **warning** · uncertifiable

The `solid-js` this project would actually import is a Solid 2 release the
checker's vocabulary was not audited on. The project is analyzed, and this
notice says the result cannot certify it.

## What it does

Dialect detection picks the language from the installed major
([unsupported-solid-runtime](unsupported-solid-runtime.md) refuses a major with
no dialect). It then asks the Solid 2 vocabulary about the exact installed
release (`rust/crates/solid-dialect/src/solid_2/releases.rs`):

| installed `solid-js` | result |
| --- | --- |
| `2.0.0-rc.3` (the audited release) and `2.0.0-rc.0` (the release most vocabulary citations were read on) | analyzed, no notice |
| `2.0.0-rc.9` | analyzed under the rc.9 vocabulary, **this notice**, naming the review's open gaps |
| `2.0.0-experimental.x` | refused with [unsupported-solid-runtime](unsupported-solid-runtime.md) |
| any other 2.x release: `rc.1`, `rc.2`, `rc.4`–`rc.8`, `rc.10` and later, betas, `2.0.0` | analyzed under the audited vocabulary, **this notice** |

The notice is one project-scoped finding located at the deciding
`node_modules/solid-js/package.json`. Its message names the installed version.
For a reviewed release its evidence lists every gap the review left open and
its hint names the review document. Every other finding is reported as usual
and still stands.

Matching is exact. A range or build-metadata spelling (`^2.0.0-rc.3`,
`2.0.0-rc.3+local`) is not the release that was read, so it gets the notice.

## Why it matters

A vocabulary answers some questions by name: which argument is a callback,
which export takes one, what TypeScript already rejects. A release nobody has
compared against it can change those answers without the checker noticing. The
rc.9 review (`docs/package-contract-v2/audits/2026-09-26-solid-2-rc9-vocabulary-review.md`)
found four such changes, and before this notice existed `--certify` reported
`certified` for an rc.9 project that wrote a store root property outside a
setter, a write rc.9 drops.

Refusing unaudited releases would turn away most current Solid 2 projects over
differences that, on the one release measured, were three additive gaps and one
typing change. Staying silent is the failure the review found. So the analysis
runs, and the notice keeps the project from being certified.

On rc.9, the one gap that needed a different vocabulary answer is closed: the
rc.9 store root is not `Readonly`, so a root property write is reported by
[no-direct-mutation](no-direct-mutation.md) there, while on rc.3 it stays
TypeScript's TS2540. The notice lists what is still open.

## How to fix

Pin `solid-js` to `2.0.0-rc.3` to certify, or use a checker release that has
reviewed your release. Without either, read the result as a report and not a
certification.

## Suppression

It cannot be suppressed per site, because there is no site. The rule options
file accepts the name, but disabling it has no effect: the notice is added
after rule enablement, as the refusal is. Passing `--dialect solid-v2`
explicitly on an rc.9 install selects the audited vocabulary instead of the
rc.9 one. That is a decision, not a detection, so no notice is added. An
unreviewed release under `--dialect solid-v2` is still the dialect detection
would pick, so it keeps its notice.
