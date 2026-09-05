# creates-decline-records

The measurement fixture for the generator's own `creates` walk
(`rust/crates/solid-reactive-ir/src/creates_walk.rs`,
`docs/adr/0008-implementation-census-for-creates.md` § "The decline records"):
one export per **blocker kind** the walk distinguishes, one per
**unresolved-callee shape** it classifies, plus one that proposes.

It is a generator-corpus fixture (`corpus.json`) and nothing here is ever
certified. What it pins is the `declinedClosures` array of the proposal refusal
sidecar (`expected-refusals.json`) — the record that says *why* no
`creates: []` proposal was made. Before those records existed the walk's answer
for an export was one bit and the reason was unrecoverable, which is why
"audit more primitives" was a guess rather than a measured choice.

## What each export is for

| entrypoint | export | walk verdict | recorded kind |
| --- | --- | --- | --- |
| `./clean` | `proposes` | proposes `creates: []` | none — the control |
| `.` | `dialectSilent` | declines | `dialect-silent { package: "solid-js", export: "createEffect" }` |
| `.` | `viaSilentHelper` | declines | `refusing-callee-fixpoint`, naming `silentHelper`'s exact declaration span, **and** that helper's own `dialect-silent` record at its own location |
| `.` | `unresolvedCallee` | declines | `unresolved-callee`, no callee identity, shape `undeclared-identifier` spelled `externalGlobal` |

## The unresolved-callee shapes, and which are pinned here

`unresolved-callee` is about half of every decline the ecosystem corpus
measures, and as one undifferentiated kind it says only "something did not
resolve". The kind now carries an
`UnresolvedCalleeShape` (`rust/crates/solid-reactive-ir/src/creates_walk.rs`),
decided from facts the build already has — no producer or Type Facts demand was
added. Its wire `kind` is still `unresolved-callee`, so every existing
`declinedClosuresByKind` count is unchanged; `shape` and `spelling` are two
appended columns.

**All seven shapes are pinned here**, one export each:

| entrypoint | export | shape | spelling |
| --- | --- | --- | --- |
| `.` | `unresolvedCallee` | `undeclared-identifier` | `externalGlobal` |
| `.` | `memberPropertyUnresolved` | `member-property-unresolved` | `publish` |
| `.` | `memberReceiverUnresolved` | `member-receiver-unresolved` | `method` |
| `.` | `computedMember` | `computed-member` | `handlers` (the receiver) |
| `.` | `parameterAliasRooted` | `parameter-rooted` | `read` |
| `.` | `expressionCallee` | `expression-callee` | `function-expression` |
| `.` | `otherSyntax` | `other` | `await-expression` |

## Why `parameterRooted` proposes since ADR 0034, and `parameterAliasRooted` still declines

`parameterRooted` (`source.read()` on the export's own parameter) is the second
control beside `clean`: it declines nothing. Until ADR 0034 it declined as
`parameter-rooted`, because the same property access is an uncensused invoking
form (`property-access-unknown-accessor`) the census refused at the
`MayExecute` floor, so a walk that proposed it would have planned a candidate
the census refused. The census now dispositions that form
`parameter-rooted-accessor` when the producer roots its subject at a plain,
unwritten parameter of the frame — the caller's object, the caller's code — so
the walk proposes exactly that shape: a direct, un-aliased parameter of the
outermost function containing the call.

`parameterAliasRooted` (one binding alias away) keeps declining under the same
`parameter-rooted` shape name, as would a nested callable's own parameter: the
producer roots neither subject, the census refuses both, and a walk that
proposed them would turn a certified row into a refused one. Pinned from the
other side by `../implementation-census-creates`, where `memberParameterRooted`
certifies and `nestedCallableParameterRead` refuses.

Nothing about the shapes is pinned by unit test alone. What the unit tests in
`creates_walk.rs` do cover, and this fixture cannot, is the shape *vocabulary*
itself — every shape's wire name, its one carried spelling, and that the kind's
own name stays `unresolved-callee`.

Two things the classifier can name but no source here produces, deliberately:

- **`expression-callee` spelled `call-expression`.** A higher-order
  `factory()()` resolves on this build — TypeScript answers an entity at the
  inner call — so the walk records no shape for it at all. The arm exists so
  that if such a callee ever stops resolving it lands in `expression-callee`
  rather than in `other`.
- **`other` spelled anything but `await-expression`.** A conditional or logical
  callee (`(flag ? a : b)()`) also resolves here: `EntitySymbols::at` answers
  with an *operand's* symbol at a compound span. `unknown-expression` is the
  honest answer for a syntax no fact table names, and no source is known to
  produce one.

There is **no `unaccepted-import` shape**. It was implemented and measured
first: an import of an unresolvable bare specifier, a deep subpath, or a
missing default still gives its local binding an alias symbol, so the callee
resolves and never reaches the unresolved branch. A namespace import's member
call (`import * as ns; ns.thing()`) lands in `member-property-unresolved` with
the receiver resolved. An unaccepted dependency is a closure hazard decided at
certification, which is a different decision from this walk's.

`proposes` is not decoration. Without an export that still proposes, an
`expected-proposal.json` with no `creates` candidate would be equally
consistent with the walk having been switched off, and the three declines
would say nothing about themselves.

**And it must live in its own entrypoint.** `index.js`'s top-level
`import { createEffect } from "solid-js"` is a bare specifier that resolves to
no *accepted* dependency, which is an `UnacceptedExternalDependency` closure
hazard: it opens every domain of that artifact case at closure replay, so no
`creates` candidate can survive there however clean the walk was. A control
sitting beside the declines would therefore show no candidate either, and
"nothing proposed" would be ambiguous between the walk and the hazard.
`clean.js` imports nothing, so its closure carries no hazard and the candidate
is visible in `expected-proposal.json` — and, since the candidate is now
published in the document too, as `proposedClosures: ["creates"]` on
`proposes`'s summary in `expected.json`. The hazard's effect is visible in the
same file from the other side: no export of `index.js` carries the label, since
opening a domain withdraws its proposal along with its closure.

That split is also the reason the decline records exist at all: on a real
consumer row the domain is open for *several* independent reasons at once, and
only the walk's own record says whether an audit would change anything.

`viaSilentHelper`'s two records are the point of reporting transitively.
`silentHelper`'s body is byte-identical to `dialectSilent`'s, and the walk is
lexical, so the export refuses only through the fixpoint over the resolved
local call edge. If the report stopped at that hop, the primitive would be
invisible in the ranking for exactly the shape real consumer packages have —
an exported entry point over module-local helpers.

## What `dialect-silent` means here, and what it does not

**It is about the dialect's canonical-primitive recognition, not about the
audited-archive tier.** `node_modules/solid-js` is a *stub*: its
name/version/integrity/manifest tuple is not the audited
`solid-js@2.0.0-rc.3`, so it could not satisfy
`solid_dialect::primitive_performs_no_operation`'s `AuditedArchive` binding
even if a row existed. That is fine, because the walk never asks that
question. The walk asks `some_audit_denies_primitive(spelling, Creates)`,
which takes a bare **name** — deliberately, because it can only gate a
*proposal* and the certifier's implementation census re-asks the identity-bound
form against authenticated bytes (ADR 0007, ADR 0008 § 2).

So this fixture pins two things and neither of them is a tier decision:

1. `createEffect` is resolved as canonical Solid 2.0 vocabulary at all — the
   stub's `2.0.0-rc.3` version is what selects that catalog
   (`rust/crates/solid-facts-backend/src/dialect.rs`).
2. That vocabulary being unaudited for `creates` produces a **named** decline
   rather than silence, with the callee's resolved package attached.

The `package` field is read from one of two *resolved* facts and never from the
callee's spelling — a wrong package here would misdirect the audit the record
exists to rank. First the compiler's own `ResolvedDeclaration::originModule`
for the callee; then, where the build has no resolved declaration, the module
specifier of the import statement this exact callee **symbol** is the binding
of. On this fixture the second answers (`"solid-js"`), which is worth knowing:
a `dialect-silent` record's package is as good as the build's binding facts and
no better. With neither the field is empty and the ranking prints
`(unresolved)`, which is a different answer from naming a package by guess.

## What must stay true

- **`createEffect` must stay a canonical 2.0 primitive with no `creates`
  denial row.** The row was withdrawn 2026-09-04
  (`rust/crates/solid-dialect/src/solid_2.rs`). *When an audit re-adds it, this
  fixture's `dialectSilent` and `viaSilentHelper` will start proposing and the
  snapshots move.* That is the correct outcome and the measurement working: the
  fixture then needs a different silent primitive (any 2.0 spelling in the
  dialect `TABLE` with no `NEGATIVE_ROWS` entry) so the kind stays covered.
- **The stub's `createEffect` signature must stay byte-faithful to the
  published rc.3 declaration**, both overloads included. A loosened stub is how
  a rule quietly starts duplicating `tsc` (AGENTS.md); nothing here depends on
  the looseness, and it must stay that way.
- **`proposes` must keep calling only a parameter, and `clean.js` must import
  nothing.** A standard-library or local-helper call would still propose, but
  the control would stop isolating "the walk ran and can clear an export"; any
  import at all reintroduces the closure hazard that made this module
  necessary.
- **`silentHelper` must stay a `function` declaration this module calls once.**
  The fixpoint follows the IR's resolved call edge (`callee_symbol` →
  `function_for_symbol`); a `const` arrow would test variable tracing instead.
- **`unresolvedCallee` must stay a bare undeclared identifier in `index.js`.**
  An import from an unaudited dependency is a closure hazard decided at
  certification, not a walk decision at all — and the entry file is JavaScript
  precisely so the undeclared global is a runtime fact rather than a `tsc`
  diagnostic this checker would then be duplicating. The same applies to every
  shape export below it: they are unresolved *because* the module is JavaScript
  with no `checkJs`, so an undeclared global, a missing property and a computed
  callee are runtime facts here, not diagnostics `tsc` would already report
  against these bytes.
- **`index.d.ts` must stay exactly as tight as `index.js`.** Every shape export
  has a declaration whose signature is the one the runtime module implements. A
  loosened declaration is how a rule quietly starts inventing a defect
  (AGENTS.md); no shape here depends on declaration slack, and the classifier
  reads `index.js`'s own syntax and entity facts rather than the declarations.
- **A record carries byte offsets, so editing this module moves the snapshot.**
  Adding a shape export shifts every later record's `location`. That is the
  cost the sidecar pin buys and it is accepted; review the moved offsets rather
  than regenerating past them.

## Not the census

The implementation census — what happens to a candidate that *was* proposed —
is `fixtures/package-contracts/implementation-census-creates`. That fixture
deliberately calls no Solid primitive, which is why it cannot express
`dialect-silent` and this one exists beside it.
