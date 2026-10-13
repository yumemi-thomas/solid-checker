# Transported member stability (ADR 0273)

StableBody is the positive direct-accessor transport; StableTracked stays clean.
Every other consumer must have no violation. Unrelated receiver writes veto
only the matching transported name: assignment, compound/logical assignment,
update, destructuring, for-of/for-in, delete, literal computed keys (including
cooked escapes), exact Object/Reflect descriptor/set/delete calls and literal
Object.assign sources. Local Object/Reflect homonyms and a local
ObjectConstructor-typed receiver do not veto stable.

Nested helper/alias mutation, second-context-consumer mutation and the review's
replaceAccount(useAccount) callback with a separate direct useContext consumer
pin the three review examples. Nested projection remains refused independently.
The exact computed receiver control confirms the existing receiver census;
unrelated nonliteral computed writes are outside the explicit premise.
WrappedLiteralVetoes covers literal writes under as const/satisfies, non-null
delete, a wrapped defineProperties descriptor key and a substitution-free
template key. Its callback-fed receiver cannot be joined by context/alias
guessing; the project-wide known-name veto must withhold each member.
Facts tests separately pin parentheses/type assertions, cooked template escapes,
dynamic keys, lone-surrogate refusal and unchanged open computed object shapes.
The unused obfuscated dotted-write/callee controls deliberately remain outside
ADR 0273's premise: their raw Unicode-escaped spellings do not veto stable.

The primitive declarations are copied from accessor-provenance's reduced
published rc.13 signatures; the claim-bearing signatures are unchanged.
The separate assign-unknown project pins the project-wide wildcard veto without
poisoning this project's stable positive. Retained/cold backend tests isolate
each spread and wrapped-key veto's addition/removal, including a callback-fed
context and a distinct direct consumer. Ordinary obligations remain visible.
