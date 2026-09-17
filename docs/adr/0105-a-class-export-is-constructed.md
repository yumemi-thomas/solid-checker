# ADR 0105: a class export is constructed, not called

Status: accepted (2026-09-14). Handshake protocol 59.

## The shape

~~~js
// @tanstack/store@0.11.1, dist/store.js — the spelling every bundler emits
var Store = class {
	constructor(valueOrFn, actionsFactory) {
		this.atom = createAtom(valueOrFn);
		this.get = this.get.bind(this);
		if (actionsFactory) this.actions = actionsFactory(this);
	}
	get() { return this.state; }
};
~~~

`Store` and `ReadonlyStore` refused with
`domain-exhaustiveness … callSignatureNotUnique`, and so did
`@tanstack/devtools-event-client`'s `EventClient`. The reason is not that the
census found something it could not decide. It is that the census never ran:

~~~go
signatures := p.checker.GetSignaturesOfType(valueType, checker.SignatureKindCall)
…
if len(signatures) != 1 {
    transcript.OpenReasons = append(transcript.OpenReasons, "callSignatureNotUnique")
    return transcript
}
~~~

A class is not callable, so `SignatureKindCall` yields **zero**, and `!= 1`
reports the same reason for "none" as for "several". The transcript returned
before an implementation was looked for.

## The construction census already existed

The 2026-09-13 depth plan filed this tier as needing "the census of a
constructor body", an ADR of its own. That census is
`classConstructorAt` (protocol 55, ADR 0047's line), and it was already
correct for the shape the corpus ships — `classHeritageAndMembers` handles
`var C = class {…}` explicitly, and applied by hand both `@tanstack/store`
classes are admitted. It was simply unreachable for a class *export*; it runs
only when some other census resolves a callee to a class.

So this premise is **construct-signature selection**, not a new census, and it
is much smaller than the plan budgeted for.

## What the producer states

The construct signatures are asked for only when the call side is **empty**. A
value with both is two claims, and picking one here would be choosing between
them without saying so.

Three things this required, each of which was a real defect in the first
version:

- **The declaration stays the class.** Selecting the construct signature lands
  on the constructor, which has no name of its own, so the transcript answered
  `"constructor"` to a query for `"Store"` — which the session refuses
  outright, and correctly: that check is what stops a producer describing some
  other node than the one demanded. It now names the class, or for the
  bundler's anonymous class expression the enclosing binding, which is the same
  indirection `const helper = () => …` already takes.
- **`classConstructorAt` is asked on this path.** Selecting the construct
  signature finds a constructor *body*, and a construction runs more than
  that: the heritage clause's constructor, every field initializer, a static
  block, a computed member name, a decorator, a parameter property. The first
  version censused the body without that gate, which would have certified a
  class whose other construction code was never examined. Its refusals are
  pinned as test cases.
- **The wire says which form it censused.** `SelectedSignature` carries no
  kind, so a consumer would have read `new Store(…)`'s census as `Store(…)` —
  an invocation the class cannot even accept. `invocation` is stated as
  `construct` and **absent for a call**, because absence is what every producer
  below protocol 59 meant by saying nothing; the certifier refuses any other
  spelling rather than reading it as either.

## A limitation, pinned rather than left to be rediscovered

A demand that lands on a class declaration's **own name** still refuses with
`callSignatureNotUnique`. The type there is the *instance* type, which has
neither a call nor a construct signature — the trap ADR 0099's comment already
records, found by the `Box` fixture — and asking the class node itself answers
the same instance type. Three attempts at a fallback failed; the refusal is now
a test case (`aDemandAtTheDeclaringNameRefuses`) rather than a surprise.

A demand at any **value** position answers: the binding of `var C = class {…}`,
or an `export { C }` specifier. The corpus's 80 `@tanstack/store` rows are the
bundler spelling and are unaffected.

## Measured

| export | rows | verdict |
| --- | ---: | --- |
| `@tanstack/store` `Store` | 40 | decidable, then **closed** by recipe |
| `@tanstack/store` `ReadonlyStore` | 40 | decidable, then **closed** by recipe |
| `@tanstack/devtools-event-client` `EventClient` | 11 | still refuses, now `implementationUnavailable` |
| `motion-utils` `SubscriptionManager` | 26 | unmeasured behind the `motion-solidjs` graph blocker |

All 80 `@tanstack/store` rows are a **single** artifact case reached from five
roots, so two recipes close every one of them.

`EventClient` moved from `callSignatureNotUnique` to `implementationUnavailable`:
the construct signature is now selected and the constructor body is not
reachable from it. That is a different question and this ADR does not answer
it.

## The veto

An empty `reads` enumeration takes no synthesized veto —
`candidate_observation` registers none, because the contradiction of
`reads: []` is a read of a source the export *owns*, which no generated module
can instrument. So these close with hand recipes, as ADR 0104's did.

The two recipes carry a proof obligation the sampled-call recipes do not.
Construction reads nothing of the caller's, so there is no owned read to count
*inside* the window, and asserting zero would be indistinguishable from a
getter that never worked. `Store`'s recipe reads its own getter from inside the
caller-supplied actions factory, which runs during the construction;
`ReadonlyStore` takes no callback at all, so its recipe reads the same getter
once *outside* the window and requires the counter to move. A run in which
that did not register is a run in which an owned read could not have been seen
either.
