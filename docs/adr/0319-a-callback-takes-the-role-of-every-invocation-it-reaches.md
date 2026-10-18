# ADR 0319: A callback takes the role of every invocation it reaches

- Status: accepted and implemented (2026-10-17), slice 1 of the forward
  invocation flow the owner allowed on 2026-10-17.
- Owners: `invocation_flow_role` and `InvocationFlow`
  (`solid-reactive-ir/src/execution_role.rs`); the per-build answer table
  `SemanticLookup::invocation_flow_answers` and the `invocation-flow-answers`
  entry of `cross_file_proof_digest` (`indexes.rs`); the boundary-manifest
  inputs of the daemon's cached snapshot (`solid-facts-backend/src/daemon.rs`).
- Supersedes: the hold in ADR 0309 ("there is no … forward-flow … in this
  slice"; the production migration "remains held") for this one question, and
  the `RetainedOptionCallback` pin in `accessor-provenance` ("definition-site
  obligations are never suppressed by a proven invocation"). ADR 0309's
  observational `value_flow` stage and its open-coverage audit are unchanged;
  this ADR adds a production fact next to it, not a migration of it.
- Relation: consulted after ADR 0315's `callee_use_site_role`, inside
  `semantic_execution_role_within`. See "Who consumes the role" for exactly
  which rules read it.

## Context

A function literal written in a component body and handed onward keeps the
body's lexical `UntrackedRendering` role unless some proof places it, so its
reads are uncertifiable SC1001 ("passed to a consumer that is not proven to
invoke it") and its parameter-member dispatches SC9012. ADR 0315 places a
literal that a callee calls through one parameter. It cannot place one that
comes back out: a handler a factory returns inside a props object that the
caller spreads onto a row, an options callback a returned closure calls, or a
callback forwarded down a recursive tree of components to an element. Those
were the M2 consumer families of the 2026-10-17 census.

## Decision

1. **Question.** For the synchronous, non-generator, non-component function
   literal directly around a read, in a closed program (ADR 0193) with no
   `eval` anywhere: the one role every invocation of that literal runs in.
2. **Walk.** The literal's value is followed forward through the positions
   that hold it, each with a member path (`["rowProps", "onKeyDown"]`):
   - a `const` (or never-written, unexported) identifier binding: every
     reference;
   - an object literal member (no getter, no computed key, no non-arrow
     function using `this` in the literal) and an object spread, unless a
     later own property of the same key overrides it;
   - a return: back to the exact call the walk entered that callee through
     (a context frame), or else to every call of the local function;
   - an argument of an exact project callee (`project_callee_at_parameter`,
     exact mapping, non-component), followed into that parameter (plain
     identifier, no default, not rest, never written, no `arguments`/`eval`);
   - a project component's attribute or spread, followed into its props
     parameter; the tag must run that declaration (current binding, exact
     member tag);
   - an intrinsic element's spread or un-namespaced `on…` attribute holding
     the value at an `on…` key: the host dispatches it as an event;
   - a conditional arm or test, and a logical operand.
   Anything else (an unknown callee, a primitive argument, a spread argument,
   a computed member, an assignment target, `new`, a module export) answers
   nothing.
3. **Invocations.** A call of the holder is an invocation. Its role is the
   semantic role at the call when that is an event, a deferred callback or a
   tracked computation; otherwise the role of every invocation of the plain
   (synchronous, non-component) function the call is written in, found by the
   same walk in the same call context. All invocations must share one role.
   `UntrackedRendering`, `UntrackedCallback`, `EffectApply`, `Unknown`, a call
   after `await`, and mixed roles refuse.
4. **Receivers.** Calling another member of a holder hands the holder over as
   `this`. The walk then needs that member's value: an arrow, a `const` bound
   to one, a non-arrow function that never mentions `this`, or a non-function
   literal. For an object literal this is its own property; an absent one
   refuses. For a component's props it is the rendering element's attribute,
   followed through `props.x` forwards to the element that rendered the
   enclosing component, context-sensitively. An attribute the element does
   not give is `undefined` only when no assignment anywhere writes a member of
   that name, nothing writes through `__proto__` or an ECMAScript prototype,
   and no reflective definition can name it (the ADR 0314 census).
5. **Recursion.** A component rendering itself again through an element the
   walk is already inside, *holding the value under the same followed prop
   path*, holds it at the same positions in the same call context, so its
   invocations are already counted. It is accepted only when every element on
   the cycle has no spread and gives each member its component calls on its
   props as an unchanged same-name forward, a value that ignores `this`, or
   (per the census) nothing. A cycle that moves the value to another prop
   (`b={props.a}`) is a new (element, path) pair and is followed again, until
   a pair repeats or the budget runs out.
6. **Completeness.** A holder already followed in the same census, call and
   rendering context, and with the same origin (the object literal or
   element chain receiver proofs read) is skipped; each census (one per
   function whose invocations are asked for) starts with an empty visited
   set, so no invocation is lost to another census. The walk is bounded (800
   holders per query); exhausting the budget answers nothing.
7. **Methods.** A function is an object-literal method only when it is the
   outermost function carrying that method name: the facts walker gives every
   non-arrow function nested in a method's body the method's name too. A
   named function expression whose own name is referenced answers nothing
   (it can hand itself on by that name).
8. **Cache.** An answer can move with any file of the program. Every
   literal's answer is computed once per build, from an empty context, into
   one table every consumer reads, and in a closed program the cross-file
   proof digest binds that table (path, literal span, role), as ADR 0302
   binds each apply's answer. A cached fragment is reused only while every
   answer is unchanged; the marker is written for every closed program, so
   switching the boundary always moves the digest.

## Who consumes the role

The role is returned by `semantic_execution_role_within`, so every consumer
that asks it for a non-write role sees it; write legality never does (the
`classifying` write marker refuses before the walk). A flowed literal is
therefore classified as if it were written inline in the event handler,
deferred callback or tracked position its invocations run in. That is checked
for each rule that can observe the difference, by checking each flowed
handler against an inline twin (`<div onClick={() => …}>` with the same body)
with the baseline and the new binary:

- SC1001 strict-read (reads, props reads): flowed and inline twins both clean.
- SC1003 no-destructure: destructuring props inside the handler is clean in
  both twins (fresh bindings per event).
- Write legality (SC2001 family): never consults the role; no finding in either
  twin.
- Async pending reads: a read in a `then` callback of the handler is clean in
  both twins.
- SC4001 missing owner (`onCleanup` in the handler): the inline twin is a
  violation; the flowed twin is silent in the baseline and with this ADR. The
  owner rules do not consume this role, so the gap is pre-existing and this ADR
  neither opens nor closes it.
- SC1007 reactive-handler-frozen judges the JSX attribute expression, not the
  handler body; this role is never asked there.

An invocation's own role is whatever `semantic_execution_role_within` already
answers at that call. That includes the dialect's `DeferredCallback` for the
callbacks of `untrack`, `createRoot` and `runWithOwner`, which actually run
synchronously in the body that calls them: a flowed callback invoked inside
one of them is classified exactly as the same code written inline there
already is. This ADR inherits that classification and does not change it.

## Review round 1 (2026-10-17)

An adversarial review found three unsound clean claims and one low case in
the first version, all reproduced and now pinned in `invocation-flow`'s
`counterexamples.tsx`: the recursion cut ignored the followed prop path
(`CyclePath`, plus `MutualCycle` across two components); a function nested in
an object-literal method was followed as the method (`NestedInMethod`); the
visited set ignored the origin receiver proofs depend on (`SeenOrigin`); and
an object spread skipped the getter and `this` checks of an own property
(`SpreadThisGetter`). The nested-method confusion also reached three older
consumers of `method_name` that grant identity: `function_binding_name`
(`owners.rs`, which named a nested anonymous function after the method),
interprocedural summary symbols (`interproc.rs`) and `functions_by_symbol`
(`indexes.rs`, protected only by insertion order), and ADR 0315's options-
literal key lookup. All now use `own_method_name`. Consumers that only refuse
on `method_name.is_some()` or require `is_none()` stay conservative and are
unchanged. All 93 corpus removals survive the fixes.

## Review round 2 (2026-10-17)

Keys were compared by their source text, so `{ "run": cb }` read as `o.run`
did not match, and the read through the quoted key was taken for another
property (`QuotedKey`). Every key comparison in the walk and in ADR 0315's
options-literal lookup now goes through one static-key helper: an identifier
name, a quoted string, a no-substitution template (computed keys only) or a
canonical decimal index, with any escape refusing; a computed member counts
only through its literal key fact (`o["run"]`, `o[1]`), and anything without
a static key keeps refusing. A receiver proof reads a literal's member only
when no spread and no key without a static name follows the property that
gives it; a getter or setter is never that value.

A function the complete census finds no invocation of now contributes no
role instead of failing the query: a call written in it never runs. That is
what makes a later quoted key replacing a spread member a proof
(`OverrideQuoted` clean; `OverrideOther`, replacing another key, stays
uncertifiable). Pinned in `counterexamples.tsx`: `QuotedKey`,
`SingleQuotedMember`, `DoubleQuotedMember`, `NumericKey`, `TemplateKey`,
`DestructuredQuoted`, `OverrideQuoted`, `OverrideOther`. The corpus result is
unchanged from round 1 (93 removed, 0 added).

## Review round 3 (2026-10-17)

Three more unsound clean claims, pinned in `counterexamples.tsx` and the new
multi-file fixture `invocation-flow-scripts`:

- **`__proto__`.** A non-computed, non-shorthand `__proto__: v` (quoted or
  not) sets the literal's prototype; it is not an own property. It now has no
  static key, so a value held there, or a receiver proof that would read a
  literal with one after the proved property, refuses (`ProtoAlias`,
  `QuotedProtoAlias`). Computed `["__proto__"]` and shorthand `{ __proto__ }`
  are own properties and keep their key. `Object.create(base)`,
  `Object.setPrototypeOf(o, p)` and class instances never become holders: the
  value is an argument to a non-project callee, or a `new` result.
- **`super`.** A method mentioning `super` uses its receiver exactly like one
  mentioning `this` (`super.m()` runs the prototype's `m` with the same
  `this`). The receiver test now reads the whole function text, parameter
  defaults included, for either word (`SuperCall`, and `SuperSetPrototype`,
  which fails closed because the holder becomes an argument).
- **Scripts.** A file without `import`/`export` syntax is a script: its
  top-level declarations are globals any file reaches by name, and a CommonJS
  file publishes through `exports`. A census of such a file's references is
  never complete. The walk refuses every holder, parameter and invocation
  census in a script, and `flow_binding_exported` treats every top-level
  binding of a script as exported (`ScriptGlobal`, `CommonJsExport`;
  `ModuleControl` is the unchanged module form). A module closure stored on
  `globalThis`/`window` is a reference in an assignment, which refuses
  (`GlobalThisRelay`). This was opened by round 2's "a function nothing
  invokes contributes no role": that rule is sound only where the census is
  complete, which is now exactly a module.

Outside this engine the "exported" notion (`module_level_exports`,
`source_function_exported`) does not model script globals either; this ADR
does not depend on those answers, and the general question is recorded in
docs/precision-backlog.md.

## Proof obligations (self-review, round 3)

Every decision the walk makes that removes an invocation or trusts an
identity, the premise it rests on, and the review cases that exercised it:

| Decision | Premise | Cases |
| --- | --- | --- |
| A member read with another key is "only read" | not a call, `new`, tag, `?.()` or chain (`.call`, `.bind`); member keys compare by static name; getters in a literal refuse; a props getter only evaluates the parent's expression, whose references the walk follows itself | c3, r2a, `QuotedKey`, `SpreadThisGetter` |
| A function "ignores `this`" | arrow; or no `this`/`super` anywhere in its text; for a literal member, the last own property with that static key, with no spread or non-static (or `__proto__`) key after it; for props, the rendering element's attribute (no spread on it), followed through unchanged forwards; absent only under the ADR 0314 census | c4, `ThisSibling`, `SeenOrigin`, `SuperCall`, `HazardRows` |
| A function "is invoked only at these sites" | the census is complete: module file; declaration not exported or written; expression not self-referenced by its name; method is the outermost function carrying its name; every reference sits in a position the walk models, anything else refuses | c2, c3d, r3c, `NestedInMethod`, `ScriptGlobal`, `GlobalThisRelay` |
| "Nothing invokes it" (no role) | the same completeness; only then is an empty census a proof | r3c, `OverrideQuoted` |
| A binding "is not exported" | module file, no export statement contains it, no export specifier names it; any other reference is itself a walked use | r3c, `CommonJsExport` |
| "Key equals key" | `static_key_text`: identifier, quoted string, backtick (computed), canonical index; escapes refuse; literal computed members through their key fact; `__proto__:` has none | r2a, r3e, `NumericKey`, `TemplateKey` |
| "Span equals span" | expressions compared after `peel_ts_sugar_span`; a JSX container counts only when one member or function is its whole text; anything unpeeled matches no branch and refuses | c1, `ForwardedRows` |
| A recursive activation "adds nothing" | same element and same followed path, same call context; every cycle element forwards called props unchanged or gives values that ignore `this` | c1, `CyclePath`, `MutualCycle` |
| A visited holder "adds nothing" | same census, call and element context, and origin | c4, `SeenOrigin` |
| A return "goes to its frame" | the callee was entered through that call; any other store of the value is a walked reference | `SpreadHandlers`, r3f |

## Consequences

49-project corpus against the post-ADR-0318 baseline (2,634 uncertifiable,
553 violations): **93 uncertifiable findings removed** (51 SC1001, 42
SC9012; 78 distinct locations), **0 added**, violations unchanged. The largest
family is app-game's browser-atlas tree: 21 keyboard-navigation options
callbacks reach the row's `onKeyDown` through `rowProps`, a factory return, a
spread, and the recursive `TreeView → TreeLevel → TreeBranch` forward. The
rest are callbacks whose every call sits in tracked JSX, an event handler or a
timer once the closure that calls them is followed (router `location`
getters behind `isActive`, test harness relays, helpers whose callbacks run in
`setTimeout`).

Coverage: the new `invocation-flow` snapshot and one moved pinned case,
`accessor-provenance`'s `RetainedOptionCallback`, whose hold this ADR
supersedes.

## Retained sessions (evidence, 2026-10-17)

Retained-versus-cold twins in
`solid-facts-backend/tests/invocation_flow_process.rs` (run in the
`backend_process` test binary with `SOLID_TYPEFACTS_BIN` set) edit one file of
a closed scratch application per generation and assert, after every edit, that
the retained `IncrementalBuilder` program and findings equal a cold build, and
that the flowed read's finding is as expected; each generation is also built
with the boundary open, where the read is always uncertifiable (the flow is
off):

1. a callee starts calling the callback in its body, and stops;
2. a consumer stops spreading the returned handlers onto an intrinsic and
   calls one in its body, and goes back;
3. a recursive component renames its forwarded prop and calls the renamed
   prop in its body, and goes back;
4. a third file flips between script and module and attaches an import to
   `globalThis`; the callee attaches its unexported helper to `globalThis`
   (which opens the helper's census, so the read becomes uncertifiable), and
   detaches it;
5. the options literal's keys are quoted, its `log` replaced by a
   `"__proto__"` key (open), and the callee calls the callback through a
   quoted computed member in its body; each step undone.

A sixth twin asks one retained builder under alternating environments
(closed/open boundary, browser/node target, CSR/SSR) across an edit; every
answer equals a cold build under the same environment. A seventh drives the
real daemon: the first version did not bind the program-boundary manifest,
so with a declared host (no host-inference inputs) flipping `package.json`
from private to published kept serving the closed, flow-proved snapshot. The
daemon now takes every `package.json` on the walk from the project up (absent
ones too) as a snapshot input; the twin checks the flip both ways, then a
callee edit both ways, retained against one-shot.

**Module resolution (review round 5).** The answer-table digest held, but
two daemon answers were stale because the retained Type Facts session
resolves imports once, when it opens: relinking a workspace package
(`node_modules/@ws/lib -> packages/ws2`) and redirecting a `package.json`
`types` field both kept the old program (the second re-analyzed, but on the
old session). Both predate this ADR; with the flow on they served no finding
where a cold run reports violations. The daemon now keeps a resolution
witness (`solid-facts-backend/src/daemon_resolution.rs`) taken from the open
program's own module graph with its import resolutions: every module file by
content; for each non-relative import, every `node_modules/<name>` and
`node_modules/@types/<name>` candidate on the walk from the importer to the
root (absent, a directory with its realpath, or a symlink with its target and
realpath, so a relink at any depth of a pnpm-style chain moves it), the
`package.json` inside each, and each candidate's `node_modules` and scope
directory; for a `#` import, every `package.json` up the importer's walk. Any
difference rebuilds the program from a new Type Facts session before the
cached snapshot is consulted. Daemon twins (retained against one-shot, then an
unchanged repeat that must be a cache hit), each under the `performance` and
`compact` idle retention levels: a workspace symlink relinked and back;
`types`, `exports` (conditional `types`) and `main` redirected each way; a
package removed and reinstalled at another version, then its installed file
edited in place; and a pnpm store link moved and back. Three of the four
fail with the witness disabled (the reinstall was already caught by the
tracked directory stamps). A dependency-source check is always one-shot (ADR
0311), so it has no retained state to go stale. Not observed: a tsconfig
the project's tsconfig `extends`, and the window between the session's
resolution and the witness's observation.

The first digest version bound every file's source hash, which made any edit
in a closed program invalidate every per-file fragment; the existing
`transported_member_helper_edits_match_cold_in_both_directions` reuse
assertion caught it. Binding the answer table instead keeps reuse wherever no
flow answer moves. The corpus is unchanged (78 locations, 0 added).

Not claimed: containers (arrays, `push`, `Map`, `For each` sources) and the
`actions` arrays of context menus (slice 3); callbacks forwarded as a JSX
child; exported factories reached other than through a call the walk entered;
members called on a holder whose value is a class instance; a vitest
`vi.mock` replacement of a component the walk resolved statically (a test
harness premise the whole call graph already shares).

Pre-existing gaps the review found in both binaries, not caused by this ADR:
a function created inside an event handler or a JSX-called method and kept
for a later component-body call (stored into a module variable, a registry
array, or through its own name) keeps the lexical role of the handler or
method (docs/precision-backlog.md).

Fixture: `invocation-flow`.
