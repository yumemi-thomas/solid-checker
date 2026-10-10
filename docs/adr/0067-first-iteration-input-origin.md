# Caller input on first loop entry

I18n's `resolveTemplate` replaces its string inside a conditional `for-of`.
The first iteration reads the caller's string; later iterations read prior
replacement results. Neither a whole-function unwritten premise nor a claim of
guaranteed execution describes that behavior.

Protocol 42 adds `firstIterationOnly` to an initial parameter-read premise.
The producer supports one synchronous `for-of`, optionally directly guarded by
an identifier/static-property condition, following the existing safe const
prefix. Its header permits only plain const identifiers/array bindings without
defaults, rest or parameter shadowing, and an identifier/property iterable or
a call with identifier/property callee and arguments. These forms cannot write
the parameter binding. The enclosing function still excludes all nested
callables, eval, arguments, parameter defaults/rest, async and generators.
External getter/iterator code therefore cannot access the lexical parameter
binding. The body must start with the existing plain assignment/leading-receiver
form. No fact is inferred about later statements or iterations.

The Rust client binds the fact to the same signature declaration and property
use. Unknown reachability is admissible only with the affirmative limitation;
unreachable uses still refuse. The verifier additionally requires the exact
callee parameter path and source occurrence, and an operation with an explicit
zero lower bound. The limited fact cannot justify a guaranteed call, an
unasserted whole-root shortcut, or a later different method. All other shape
and behavior obligations remain independent. No external contract or trust
authority is inferred from the header call's name.

The [ten-probe measurement](../package-contract-v2/phase21/2026-09-08-first-iteration-recovery.md)
recovers three I18n root cases and three complete rows. All 13 prior
Marker/Motion/Utils selections and claims remain identical. Producer,
native/verifier and client tests pass; full verification passes with exit 0,
TOTAL 240.40s and no failed-step marker. Both ordinary-CLI replacement controls
still refuse. The full corpus has not been rerun, and no denominator changed.
