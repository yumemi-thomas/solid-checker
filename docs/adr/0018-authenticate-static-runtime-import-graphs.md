# 0018 — Authenticate ordinary static runtime imports in dependency graphs

Status: accepted and implemented; measured outcomes in docs/2026-09-04-published-js-probe-unlock.md
Date: 2026-09-04

ADR 0017 makes the 20-node Kobalte 0.9.2 re-export graph certifiable, but its
two root JavaScript cases still carry no creates closure. The graph adapter
selects only runtime re-exports for semantic dependencies. Ordinary runtime
imports become compiler sources, which authenticate typings but cannot supply
accepted semantic dependency edges. Closure replay correctly leaves every
affected behavior domain open.

Include static runtime imports alongside re-exports in the existing graph
acquisition path. Preserve exact importer occurrences, installed copy and lock
selection, conditions, archive authentication, dependency-first generation and
native reconstruction of every edge. Declaration imports remain compiler
sources; dynamic imports keep their existing open disposition. Cycles,
ambiguous copies, missing archives and runtime-library policy gaps still refuse.

This does not grant authority to generated dependency proposals: the complete
graph still has to pass native verification before any receipt is issued.
Removing the runtime-import hazard without authenticating its dependency would
weaken the closure and is rejected. Treating compiler-source acquisition as
semantic acceptance would conflate two separate authority channels and is
also rejected. Expanding acquisition is preferable because it supplies the
missing evidence to the existing verifier rather than bypassing a demand.

Sandbox scheme 6, census and mandatory veto policy do not change. Measure the
same offline root graph, with zero cache misses, before adding any recipes.
An accepted open graph alone is not a new creates closure.
