# unaudited-solid-release

`SC9014` · **warning** · uncertifiable

The Solid 2 packages this project would actually import are an installation
the checker's vocabulary was not audited on. The project is analyzed, and this
notice says the result cannot certify it.

## What it does

Dialect detection picks the language from the installed `solid-js` major
([unsupported-solid-runtime](unsupported-solid-runtime.md) refuses a major with
no dialect). It then resolves the three packages Solid 2 ships as, each the
way the code that imports it resolves it, and asks the Solid 2 vocabulary
about that installation (`rust/crates/solid-dialect/src/solid_2/releases.rs`):

- `solid-js`, the nearest `node_modules/solid-js/package.json` above the
  project;
- `@solidjs/signals`, from the installed `solid-js`'s real directory, because
  `solid-js` re-exports the reactive core (and the store typing) from it;
- `@solidjs/web`, from the project.

Each release-dependent answer is taken from the package that declares it: the
store root typing, `omit`'s predicate form and `flush`'s action-step throw
from `@solidjs/signals`, `dynamic`'s options from `@solidjs/web`, and `until` from `solid-js` and
`@solidjs/signals` together. Every `solid-js@2.0.0-rc.N` depends on
`@solidjs/signals: ^2.0.0-rc.N`, a range, so a fresh install of
`solid-js@2.0.0-rc.3` resolves `@solidjs/signals@2.0.0-rc.9` today; reading
`solid-js` alone gave that install rc.3's store answer on bytes that declare
rc.9's.

| release, per package | result |
| --- | --- |
| `2.0.0-rc.9` (the audited release) | analyzed with rc.9's answers, no notice, except the `solid-js` re-export gap where the project reaches one of its five names (below) |
| `rc.0`-`rc.3`, which behave as rc.3 on every premise the dialect cites | analyzed with rc.3's answers, **this notice**: older than the audited release, so new rules and precision work are measured on rc.9 only |
| `rc.4`-`rc.6` | analyzed, **this notice**: older than the audited release, and `solid-js` re-exports `registerPatch`, `registerRowOps` and `registerSlotPatch`, and `@solidjs/web` exports `installListDriver` and `driveList`, callback-taking exports the vocabulary neither models nor excludes |
| `rc.7`, `rc.8` | analyzed with rc.9's store answer (a root write type-checks), and on rc.8 rc.9's `FLUSH_IN_ACTION` answer, **this notice**: older than the audited release, and no negative row is granted for their `@solidjs/signals` |
| `2.0.0-experimental.x` of `solid-js` | refused with [unsupported-solid-runtime](unsupported-solid-runtime.md) |
| any other version (`rc.10` and later, betas, `2.0.0`, an inexact spelling) | analyzed with the conservative answers below, **this notice** naming the package as not compared |
| `@solidjs/signals` does not resolve | the conservative answers, **this notice** |
| `@solidjs/web` does not resolve | no notice: nothing can import `dynamic`, the one answer it owns |

Packages at different releases are judged answer by answer, each from its
owner, and the notice adds a gap saying no review read that combination
(rc.0-rc.3 count as one release for this).

**A gap scoped to named exports.** Most gaps are about the installation,
whatever the project imports. One is about five names: `solid-js@2.0.0-rc.9`'s
typings re-export `createErrorBoundary`, `createLoadingBoundary`,
`createRevealOrder`, `sharedConfig` and `$DEVCOMP` from files that no longer
declare them, so under `skipLibCheck` those names are untyped and a call
through one is not the primitive. That gap is due only for a project that
reaches one of them from `solid-js`: a named or aliased import (type-only
included), a namespace member read (`S.createErrorBoundary`,
`S["createErrorBoundary"]`, `<S.Name>`), or a project module's
`export { … } from "solid-js"`. The gap's sentence then names what reached it,
and an evidence step locates each site. A use that does not say which names it
reaches keeps the gap due: a namespace object that escapes (passed, stored,
destructured, re-exported, named in a type, or indexed by a non-literal key),
a default import, `export *` or `export * as` from `solid-js`, and a literal
`import("solid-js")` or `require("solid-js")`. A `paths` alias the compiler
resolves into the `solid-js` package counts as `solid-js`; a subpath
(`solid-js/internal`) re-exports none of the five and does not. A nonliteral
`import(path)` does not count: it resolves through no declarations on any
release, so it loses what it loses whether or not the gap is open. A project
that reaches none of the five gets no notice from this gap, and on the rc.9
triple, the audited release, no notice at all.

**The conservative answers.** An unknown or unresolved `@solidjs/signals`
keeps the `Readonly` store answer, so [no-direct-mutation](no-direct-mutation.md)
does not report a write to a store root's own property, and the notice says
so. Reporting it would be unsafe: every signals release before rc.7 declares
`Store<T> = Readonly<T>`, where `tsc` reports that write as TS2540, and the
checker never reports what TypeScript already reports. `until` is not a name
the vocabulary knows unless both `solid-js` and signals are at rc.5 or later,
so [until-in-tracked-scope](until-in-tracked-scope.md) cannot fire on an
installation where the import is TS2305. `flush` is not claimed to throw
inside an action step, so [flush-in-action](flush-in-action.md) is silent. A
`dynamic` call with options on an unknown `@solidjs/web` is the form that
states nothing, and on rc.0-rc.8 it is the default form whatever it passes,
because those runtimes never read the option.

The notice is one project-scoped finding located at the deciding
`node_modules/solid-js/package.json`. Its message names the version of each
package found, its evidence names each manifest and every open gap, and its
hint names the review documents. Every other finding is reported as usual and
still stands.

Matching is exact. A range or build-metadata spelling (`^2.0.0-rc.3`,
`2.0.0-rc.3+local`) is not the release that was read, so it gets the notice.

## Why it matters

A vocabulary answers some questions by name: which argument is a callback,
which export takes one, what TypeScript already rejects. A release nobody has
compared against it can change those answers without the checker noticing. The
rc.9 review (`docs/package-contract-v2/audits/2026-09-26-solid-2-rc9-vocabulary-review.md`)
found four such changes, and before this notice existed `--certify` reported
`certified` for an rc.9 project that wrote a store root property outside a
setter, a write rc.9 drops. The rc.1-rc.8 review
(`docs/package-contract-v2/audits/2026-09-26-solid-2-rc1-rc8-release-review.md`)
placed the releases between, and found that the store typing follows
`@solidjs/signals`, not `solid-js`.

Refusing unaudited installations would turn away most current Solid 2 projects
over differences that, on the releases measured, were additive gaps and one
typing change. Staying silent is the failure the review found. So the analysis
runs, and the notice keeps the project from being certified.

Where a release changes an answer, the answer follows the installed package.
From `@solidjs/signals` rc.7 the store root is not `Readonly`, so a root
property write is reported by [no-direct-mutation](no-direct-mutation.md)
there, while through rc.6 it stays TypeScript's TS2540. rc.9's
`omit(props, predicate)` invokes its predicate on every read of the returned
view, so a predicate not proven inert is reported by
[reactive-dispatch-unresolved](reactive-dispatch-unresolved.md) there, while
earlier `omit`s never invoke an argument and their key lists stay values.
rc.9's `dynamic(source, { static: true })` runs its source once, untracked,
under the caller's owner, while earlier `dynamic`s ignore the option. The
notice lists what is still open.

## How to fix

Pin `solid-js`, `@solidjs/signals` and `@solidjs/web` to `2.0.0-rc.9`, the
audited release (since 2026-09-27; it was rc.3 before), to certify, and do not
import the five names above from `solid-js`. `@solidjs/signals` is a dependency of `solid-js`, and its range admits
later releases, so pin it with an `overrides` (npm, bun), `pnpm.overrides` or
`resolutions` (yarn) entry. The notice's hint names the versions it found. Or
use a checker release that has reviewed your installation. Without either,
read the result as a report and not a certification.

## Suppression

It cannot be suppressed per site, because there is no site. The rule options
file accepts the name, but disabling it has no effect: the notice is added
after rule enablement, as the refusal is. Passing `--dialect solid-v2`
explicitly selects the audited vocabulary instead of the one detection built.
That is a decision, not a detection, so no notice is added. An installation
detection answers with the audited vocabulary anyway (an rc.4 triple, an
unreviewed release) is still the dialect detection would pick, so under
`--dialect solid-v2` it keeps its notice.
