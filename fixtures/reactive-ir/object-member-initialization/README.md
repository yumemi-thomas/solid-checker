# ADR 0299 review: tdz

App calls a closure before its function-local receiver initializes, catches the
ReferenceError, then initializes the object without calling it. Its original
cleanup never registers; ba3d9085d nevertheless reports a violation. SharedCase
and the static import cycle exercise the same initialization boundary.
module-tdz covers an early module call even without a cycle. Captured is also
refused conservatively; Initialized retains its proven cleanup violation.
CallbackTdz also keeps its read uncertifiable: the refused method cannot supply
callback timing through the older method-declaration fallback. cycle-a keeps
its exported definition-site obligation beside the initialized control.

Solid signatures are copied verbatim from object-member-import's published
rc.13 declarations. All sources pass tsc against the real rc.13 typings.
