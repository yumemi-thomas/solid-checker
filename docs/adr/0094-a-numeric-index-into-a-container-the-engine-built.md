# ADR 0094: A numeric index into a container the engine built

- Status: accepted and implemented (2026-09-12); written with the
  implementation
- Date: 2026-09-12
- Owners: Type Facts producer (`engine_indexed_containers.go`,
  `uncensused_invoking_forms.go`)
- Relation: `engineOwnedIterableContainers`' argument applied to the index
  signature instead of `Symbol.iterator`. Handshake protocol 53 → 54.

## Context

§ 76 chased the `written-parameter` leg into `@corvu/utils::combineStyle` and
found that neither it nor the `local-binding-written` leg behind it was the
refusal that decides. The read that actually stops the argument is:

```js
var extractCSSregex = /((?:--)?(?:\w+-?)+)\s*:\s*([^;]*)/g;
function stringStyleToObject(style) {
  const object = {};
  let match;
  while (match = extractCSSregex.exec(style)) {
    object[match[1]] = match[2];   // ← `match[1]`
  }
  return object;
}
```

`accessorFormLocked` asks the compiler to name the member. A numeric literal is
one of the two key kinds it can ask about, and the compiler answers nothing —
because `RegExpExecArray` reaches its elements through `Array<string>`'s **index
signature**, and an index signature declares no property symbol. So the read
arrives as `property-access-unknown-accessor`, exactly as a computed key does,
and the subject-root walk that then answers `written-parameter` or
`local-binding-written` is only the diagnostic for a form already recorded.

§ 77 measured what that costs by reading all 33 element-access refusal sites out
of the published artifacts: **18 carry an exact numeric-literal key**, and the
nine withheld `creates` exports whose *every* refusal is one of them carry 112
withheld closure entries across 12 probes.

## Decision

An element access records **no form at all** when

- the key is an **exact numeric literal** — the compiler resolves a property
  from a string or numeric literal and from nothing else; and
- the access is a **read**, not an assignment target; and
- **every constituent** of the subject's apparent type is an *engine-owned
  indexed container*: a reviewed default-library interface whose numeric index
  reaches an own data property of an object the engine itself allocated.

The table is `Array`, `ReadonlyArray`, `String`, `IArguments`,
`RegExpExecArray`, `RegExpMatchArray`, `TemplateStringsArray` and the typed
arrays, and every declaration of the named symbol must be the default library's
— the all-declarations quantifier `isDefaultLibraryMemberLocked` already
applies, which refuses a user interface called `Array` and a `declare global`
augmentation of the real one.

The premise has two halves and needs both. A default-library index signature
*declares* no accessor — TypeScript cannot express one there — but a declaration
is evidence about the bytes that run only when the object is the engine's own.

## What refuses, and why each is a refusal rather than a skip

- **A structural interface**, and this is the whole precision of the table.
  `ArrayLike` and `ConcatArray` declare `readonly [n: number]: T` in the very
  same library file, and both are contracts an ordinary object satisfies while
  carrying whatever getter it likes — so the index named by the declaration is
  not the index that runs. This is `Iterable` versus `Array` in
  `engineOwnedIterableContainers`, and `PromiseLike` versus `Promise` before it.
- **Every DOM and web-worker indexed collection** — `NodeList`,
  `HTMLCollection`, `DOMTokenList`, `FileList`. Their indices are engine code in
  fact; they were not reviewed here, and "the browser probably owns it" is not a
  premise.
- **A user type with a numeric index signature.** Same argument as the
  structural case, without even a library declaration behind it.
- **A computed key.** The premise is about a member the compiler can name. A
  string-literal key is excluded too: it names a *declared* member, and one that
  resolved to nothing is precisely the unresolved case — admitting it would read
  "the checker found no member" as "no member is reached", which is the failure
  `awaitFormLocked`'s comment names.
- **Write position.** A write to an engine array's index runs no setter either,
  but `SubjectWrite` is load-bearing for the `writes` and `invalidates` domains
  and dropping the row would drop that mark. Refused rather than modelled; none
  of the 18 measured sites needs it.
- **A union with any constituent outside the table**, including `null` and
  `undefined`. That is stricter than the argument requires — reading an index of
  either throws a `TypeError` and runs no user code — and it stays strict
  because "the form throws" is a claim about the whole form rather than about
  the member, and belongs to whatever decision wants to make it.

One case is admitted that looks like it should not be: `value![0]` behind a
non-null assertion. `GetTypeAtLocation` answers the narrowed type, so the null
constituent is gone before this premise sees it. Admitting it is sound whether
or not the assertion holds — if the value really is null at run time, the read
throws before any user code could run — but the mechanism is the checker's
narrowing rather than this premise's quantifier, and
`assertedMatchElement` pins it so that is not rediscovered as a hole.

Two limits carry over unchanged from the iterable table, and they are the two
every declaration-based premise in that file carries: a value whose static type
is `Array<T>` while the runtime object is a **subclass** overriding the index
answers from the base declaration, and a **Proxy** is outside every producer
census.

## Consequences

This *removes* rows a consumer previously received, and silence on this census
is a positive claim, so protocol 53 → 54: a consumer that reviewed only 53
would read the new absence as a weaker statement than it is.

`engineIndexRead` and `engineModuleIndexRead` certify in
`fixtures/package-contracts/implementation-census-creates`, with
`engineComputedIndexRead`, `userIndexRead` and `arrayLikeIndexRead` refusing
beside them. Every subject in that block is one **no other premise roots** — a
rest parameter, which ADR 0034 explicitly excludes, or a module binding
initialized from a call, which ADR 0044 refuses — because the first draft of
this fixture read plain unwritten parameters and all five cases closed on
ADR 0034 whatever this premise said. A control that would pass without the
change under test is not a control.

**On the corpus: 5,275 → 5,389 certified closures, +114**, with withheld falling
11,552 → 11,438 and 14 rows moving, every one of them upward. § 77.3 priced it
at 9 exports and 112 closure entries as an upper bound: the total came in within
2%, the export list 44% right.

Four exports closed — the three `combineStyle` copies and
`@solid-primitives/i18n::resolveRichTemplate` — and they carry all 114 because
`combineStyle` is published verbatim by eight packages. Four more had their
predicted refusal removed and stopped at a blocker behind it that no report
could have shown beforehand (`split` at a computed `_list[i]` in the same
function, `filterInstance` and `filterOutInstance` at the callee `ofClass`,
`compose` at a call through a nested callable's parameter), and one is
undetermined because its rows stopped covering `motion-dom` this run.

Seventeen refusal sites went away and one arrived: `_list[i]`, the form the walk
reaches once `list[0]` stops refusing. That is the census's normal inward motion
— it refuses a domain on the *first* premise it cannot establish, so every
refusal hides what is behind it — and not a regression. § 78 of
`phase21/2026-09-10-reads-veto-observation-design.md` carries the tables, and
§ 78.3 the benchmark-churn caveat that any per-export diff needs.
