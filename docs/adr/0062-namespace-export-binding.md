# Exact namespace export entities

`export * as Bucket from "./member"` exports a module namespace, even when
`./member` also exports a callable named `Bucket`. The generator's entry export
entity lookup previously traversed that syntax as a bare export star and could
select the member's callable identity. Native certification correctly refused
the resulting callable claim about the namespace.

Normalized export facts now retain the exact namespace binding span separately
from the namespace name and from named re-export specifiers. The demand planner
requests symbol, runtime identity, type and signature facts at that span. Both
indexed and ordinary entry-entity lookup select that exact entity; only a bare
export star may recurse by member name, and only when the existing export
precedence check says no explicit binding overrides it. This also prevents an
earlier star from winning over a later explicit namespace or numeric export.
A direct namespace export with no exact
compiler entity refuses generation instead of adopting a same-named member.

The existing runtime-kind reconciliation consumes the namespace entity's
positive non-callable/non-constructable facts and discards an incompatible
inferred callable summary. No new behavior axiom, Type Facts wire field,
receipt format or trust policy is introduced. The normalized AST field is
optional on deserialization; absence supplies no binding span or proof.

The `namespace-reexport-identity` generator fixture covers a same-named callable
member, a renamed namespace, named and bare-star callable controls, explicit
namespace/numeric overrides of an earlier star, and a missing-module refusal.
It pins six generated artifact cases and the exact
refusal census. The normalized-fact test pins the namespace binding's byte span.
Kobalte Core's published `./src/index.tsx` is the motivating remaining case;
its certification gain is measured separately rather than inferred from this
proposal correction.
