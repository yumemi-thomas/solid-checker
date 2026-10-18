# use-site-role

**Claim (ADR 0315).** Code directly in a function literal that a project
callee runs only through one parameter takes the one role every call of that
parameter (or of its property) runs in, classified in the callee's file
(`execution_role::callee_use_site_role`). The literal is the argument itself,
a data or method property of a closed options literal that is the argument,
or a project component's prop. The callee may read other properties, test
the property (`p.f ? … : …`, `p.f && …`), call it, or forward it as exactly
another project component's prop that the same proof places. Every call must
be an event, a deferred callback or a tracked computation, the same for all,
reached only through functions handed as themselves to a call or a JSX
attribute. Write legality never asks this arm.

| Case | Finding | Why |
| --- | --- | --- |
| `UsesMemo`, `UsesEffectCompute`, `UsesDeferred`, `UsesLabel`, `UsesModal` (`active`) | none | the callee runs the literal only in a memo, an effect's compute, a timer, tracked JSX (the per-call-site summary path already proved these) |
| `UsesOuter` | none | `Outer` tests `text` and forwards it to `Label`, which calls it in tracked JSX: this arm's case |
| `UsesModal` (`onEscape`) | `SC1001` uncertifiable | `onEscape` runs only from the returned handler |
| `UsesModuleState` | `SC1001` uncertifiable | the hook calls the absent `ignore` with the options object as `this`; an inherited `Object.prototype.ignore` could run any property |
| `UsesList` | `rows`'s read `SC1001` uncertifiable | `rows` runs from a returned closure |
| `UsesApply` | `SC1001` uncertifiable | an effect's apply callback reports untracked reads (ADR 0321 resolves the dispatch to the literal's arrow) |
| `UsesBody` | `SC1001` uncertifiable | the callee calls it in its body (ADR 0321 resolves the dispatch to the literal's arrow) |
| `UsesMixed` | `SC1001` uncertifiable | a memo and a timer: two roles |
| `UsesGetter` | `SC1001` uncertifiable | a getter in the options literal |
| `UsesSpreadLiteral` | `SC1001` uncertifiable | a spread in the options literal |
| `UsesEscape` | `SC1001` uncertifiable | the property is handed to a sink |
| `UsesSpreadProps` | `SC1001` uncertifiable | the component spreads its props |
| `UsesWrite` | `SC2001` violation, as before | a write's role is classified without this arm |

Calling another property of the parameter (`p.g()`, a tag, `p.g.call`) hands
the object over as `this`; the caller's value for it must be an arrow, a
`const` bound to one, or a non-function literal. A method or function
expression anywhere in the options literal, or as a prop value, refuses for
the same reason, and so does a spread on the element.

`solid-js.d.ts` and `node_modules` are copied from `forwarded-tracked-compute`,
with `createEffect` and its types added from `accessor-provenance`'s stub
(published rc.13 `@solidjs/signals` declarations) and a `span` element. The
sources pass `tsc --noEmit` with the fixture stub.
