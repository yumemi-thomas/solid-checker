# ADR 0315: A literal takes the role its callee runs it in

- Status: accepted and implemented (2026-10-17).
- Owners: `callee_use_site_role`, `literal_use_site`,
  `callee_parameter_use_role`, `sibling_ignores_receiver` and
  `receiver_function_at` (`solid-reactive-ir/src/execution_role.rs`).
- Relation: generalizes ADR 0199 (event-only props) and ADR 0200 (tracked
  compute of a direct argument) to options-object fields and to tracked and
  deferred props; consulted after both, never by write legality.

## Context

A function literal written in a component body and handed to a project
callee keeps the body's lexical `UntrackedRendering` role unless something
proves where the callee runs it; its reads are then uncertifiable SC1001
("passed to a consumer that is not proven to invoke it"). Several callees run
such a literal only inside a memo or effect compute, a host timer, an event
handler or tracked JSX.

## Decision

1. **Position.** The literal is exactly the argument at a slot of an exact
   project callee (`project_callee_at_parameter`), a data property of a
   closed options literal that is exactly such an argument (no getter,
   spread, computed or duplicate key, and no method or function expression
   anywhere in it), or a project component's prop (no spread on the element,
   no non-arrow function prop value).
2. **Uses.** The callee is synchronous; the parameter is one identifier with
   no default, not rest, never rebound, with no `eval` or `arguments` in the
   callee. Every reference is the object of a non-computed member: another
   property's read; a test of the literal's property (`p.f ? … : …`,
   `p.f && …`); a call of it; or exactly another project component's prop
   that this proof places in turn. Calling, tagging or reaching through
   another property hands the object over as `this`, so the caller's value
   for it must be an arrow, a `const` bound to one, or a non-function
   literal; an absent property proves nothing (`Object.prototype` may carry
   one).
3. **Role.** Each call's role is classified in the callee's file. Every
   function between the callee and the call is handed, as itself, to a
   primitive, a standard-library call or an intrinsic attribute, and holds
   the call in its body (not a default initializer); none is async or a
   generator. Every call must be an `EventCallback`, `DeferredCallback` or
   `TrackedJsx` role, the same for all; that role is the literal's.
   `EffectApply`, `UntrackedCallback`, body and mixed roles refuse.
4. **Writes** never ask this arm (a `classifying` marker), so write legality
   keeps its previous role.

## Consequences

49-project corpus: 4 uncertifiable findings removed (3 SC9012 dispatch
obligations whose call sits in a literal a router hook runs only in a tracked
compute, 1 SC1001), none added, 553 violations unchanged. A first version
without the receiver and nested-function conditions removed 20 on the corpus
but also cleared three pinned cases in `forwarded-tracked-compute`
(`DefaultMember`'s default initializer, the receiver-exposing
`RoutedSiblingReceiver`/`RoutedSubjectReceiver`) and `project-callback-uses`'
`SiblingNamedInvoke`/`SiblingAbsent`; those refusals are why the yield is
small. Most corpus callees call an optional sibling (`options.ignoreEscape?.(e)`)
the caller leaves absent.

Fixture: `use-site-role`.
