# ADR 0116: A return that hands back what the caller's argument returned

- Status: accepted and implemented (2026-09-24); written with the implementation
- Date: 2026-09-24
- Owners: the Type Facts producer's return arms (`invocation_transcripts.go`,
  handshake protocol 62), the generator's argument-container walk
  (`returns_walk.rs`, `demand_plan.rs`, `inferred_contract.rs`), the semantic
  model's `invocation-result` value shape, the policy-2 `returns` census,
  positive fact and synthesized veto (`type_facts.rs`, `synthesized_vetoes.rs`),
  and the consumer's `returns` projection (`contracts.rs`)
- Relation: widens ADR 0115's enumeration by one kind of value. Everything
  ADR 0115 decided about one `return` per value, the census, the veto and the
  consumer's reading of a union holds unchanged.

## Context

After ADR 0115, `@solid-primitives/utils`' `accessWith` (27 sites) was the one
export whose only open consumer domain was `returns` with a shape in reach:

```js
function accessWith(valueOrFn, ...args) {
  return typeof valueOrFn === "function" ? valueOrFn(...args) : valueOrFn;
}
```

`access` (157 sites) has the same shape, `typeof v === "function" && !v.length
? v() : v`. Both were proposed ADR 0113's plain return and refused, correctly:
the completion is whatever the caller's value is, or whatever calling it
returns. ADR 0115 can say the first; nothing could say the second.

## Decision

**A return may hand back the value an invocation of the caller's argument
returned: `invocation-result i`, the completion of calling the caller's
argument at `i`, handed back unchanged.** It is one more exact output beside
`parameter i` and `argument-array [i, …]`, enumerated the same way: one
`return` operation per value, closed by the `returns` census from the
producer's arms.

### The shape

`ValueShape::InvocationResult { parameter }`, canonical tag 19, wire
`{"kind": "invocation-result", "parameter": i}` with `parameter` required. It
names no arguments of the invocation: the value is what the call returned,
and which arguments produced it is the `callbacks` domain's business, not the
return's. It carries no knowledge set, so it has no closure of its own.

### The producer states it (protocol 62)

`ReturnArm::invoked` is the unchanged whole input binding a call arm invokes:
the arm, after identity-preserving wrappers, is a call expression -- not
optional (`f?.()` hands back `undefined` when `f` is nullish), not `new`, not a
tagged template -- whose callee, after the same wrappers, is an identifier the
producer's `unwrittenParameterIdentity` names. A returned call of such a
parameter is now decomposed as a one-arm root, so `return f()` states its arm
exactly as a returned conditional does. Nothing else changes: present means
exhaustive, and the bounds are ADR 0115's.

### What the census decides

`arm_container` maps an arm with `invoked` to `ArgumentContainer::Invocation`,
and the census premises of ADR 0115 hold as written: every live value is a
container the claim enumerates, and every enumerated container is handed back.
A lone `invocation-result` claim is admitted (it is not ADR 0075's lone whole
parameter). An arm that states `invoked` beside `arrayLiteral` or `parameter`
is a producer disagreement and names no container.

### The generator proposes it

`returns_walk::argument_container_return` reads a returned call -- the whole
completion or a conditional's branch -- whose callee is an identifier resolved
by symbol to one of the function's own whole parameters, and the demand plan
asks for that callee's symbol. A `new` starts before its callee and is not
read; an optional call is proposed and refused by the census by name, because
the Oxc call fact does not say it is optional. The walk now proposes one
container alone when it is not a whole parameter, so `(fn, value) => fn(value)`
and `(value) => [value]` are proposed too; for the second the demand plan now
asks for an expression-bodied arrow's array elements, which a block's
`return [value]` already had. And the proposal is made wherever the reactive
analysis left the return undescribed: `Known(None)` for a union whose branches
disagree, as in ADR 0115, and now `Open`, which is how it reads a lone array of
a parameter.

### The veto

A claimed invocation slot is sampled with recording functions of arity zero
and one, each returning a fresh token, beside the identity samples ADR 0115
takes; the `return-outside-containers` marker fires when a normal completion is
none of the claimed arguments, arrays of them, or tokens a recording function
returned during that sample call. SameValue and the token list are read
without a mutable intrinsic.

### The consumer

`invocation-result` is exact and names no leaf the consumer can project, so it
reads the way a local call of a parameter reads: no reactive return it can
name. In a union it is part of ADR 0115's rule unchanged -- a closed claim of
exact outputs without one shared leaf is `Known(None)` -- and alone it is the
same answer. That can hide a store a callback returns and never invents one.

## What still refuses

- `access` stays at some uses: its `callbacks` closure is refused because
  `!v.length` reads a property of the caller's value, which may run a getter
  (ADR 0100 rule 2). Its `returns` now closes.
- An optional call, a member call of the argument (`o.f()`), `new f()`, a
  tagged template, and a call of a written parameter.
- A logical `||`, `&&` or `??`, and a lone `return []`, whose return fact records
  no elements: the walk reads neither, and nothing measured asks for them.

A correction to ADR 0115: a parenthesized returned conditional *is* read. The
Oxc facts are parsed with `preserve_parens: false`, so the conditional is the
return's expression itself; `implementation-census-argument-returns`'
`parenthesized` pins it.

## Consequences

- Handshake protocol 61 -> 62: a rebuilt `bin/solid-typefacts` and new
  certification pins. No existing document's digest moves.
- Contract corpus: four exports that return a call of their parameter
  (`callback-reactive-arguments`' `mapValue`, `conditional-targets`'
  `maybeRead`, `destructured-parameter-callback`'s `Parameter`,
  `implementation-census-reads`' `invokesCallerAccessor`) propose an
  invocation result instead of ADR 0113's plain return; the tracer fixture
  gains `accessWith`, `access`, `run`, `wrap`, `parenthesized` and the
  refused `optionalCall`. Possible operations 470 -> 484, proof candidates
  1,581 -> 1,605.
- The tracer certifies all but `optionalCall` against the real producer
  (`the_argument_container_census_certifies_exactly_the_enumerated_containers`),
  and `package-argument-container-consumer` gains `accessWith` and its open
  control (coverage 84 projects, 444 findings).
