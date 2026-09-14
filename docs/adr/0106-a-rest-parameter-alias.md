# ADR 0106: a spread of a rest-parameter alias

Status: accepted (2026-09-14). No protocol change: the producer states no new
fact, it stops recording a form.

## The shape

~~~js
// @solid-primitives/utils@6.4.1
export function createMicrotask(fn) {
    let calls = 0;
    let args;
    onCleanup(() => (calls = 0));
    return (...a) => {
        (args = a), calls++;
        queueMicrotask(() => --calls === 0 && fn(...args));   // dist/index.js:4711..4718
    };
}
~~~

> `reads-census premise required: the iteration-protocol form (SpreadElement)
> … states no reviewed subject root, so whose value it reads is undecided`

62 recipe-less `reads` rows on the 2026-09-14 pin.

## This is an existing argument followed one hop

`isUnwrittenRestParameterReferenceLocked` already clears a spread of a rest
parameter's **own** binding, and its reasoning is the whole premise: the
specification builds a rest array with ArrayCreate at every call, so it is an
ordinary Array — never a Proxy, never an object a caller shaped — and spreading
it reaches `Array.prototype[Symbol.iterator]` and nothing else. That is the
standing the census already gives every default-library member, not a new claim
about the caller.

What the corpus writes is not the direct spread but an alias of it. `args` is a
binding of the *outer* function that the inner arrow assigns the rest array to,
which is what every compiled debounce-alike emits. So this follows the same
argument across exactly one assignment.

The type cannot answer this, which is why it is a syntactic check. An untyped
rest parameter is `any[]`, and an `any[]`-typed value need not be an array at
all.

## What makes "every value" real

`bindingValueSourcesLocked` answers every expression the binding can take its
value from — its initializer and the right-hand side of each plain assignment —
**or false**, when some write is one it cannot read a single value out of: a
compound assignment, an update expression, a destructuring target, a `for…of`
head. Each of those refuses the whole binding rather than being skipped, so a
source left out cannot quietly become a claim about some of the values.

A binding with no initializer and no assignment holds `undefined`, whose spread
throws before any lookup and so reaches no user code. It is refused anyway:
clearing a form on a value that can only throw states nothing worth stating,
and an empty source list is likelier to mean the walk saw nothing than that the
code does nothing.

**One hop, deliberately.** A binding assigned from another such binding is a
join this does not attempt — each hop is another place a write could be missed,
and the corpus shows no case that needs it.

## Measured

`createMicrotask` moves from `census refused` to decidable against the real
`@solid-primitives/utils@6.4.1` artifact, certified through
`@kobalte/utils@0.9.2` in the graph lane. 62 rows, which close with a hand
recipe as ADR 0104's and 0105's did — an empty `reads` enumeration takes no
synthesized veto.

## No protocol change, and why that is the right call

The producer states no new fact here. It stops recording a form it used to
record, under an argument the codebase already carries for the direct case.
AGENTS.md ties a protocol bump to the producer *stating a new fact*, and this
states none.

What still forces a matched rebuild is the pin: the verifier carries the
producer's source-manifest digest, so a checker built against the old producer
refuses this one outright. The pairing is enforced by the mechanism that exists
for it rather than by a number that would mean nothing on the wire.
