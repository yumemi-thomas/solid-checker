# declared-served-closure

Claim (ADR 0890): a closure run on the creating pass's stack is no cut: `served_in_creating_pass` holds there as for a direct read (ADR 0632), so a declared `loadingValue` memo's read in an IIFE keeps its SC1001 violation, with no pending row beside it. The open twins (an apply, a helper a later call runs) are in `rc21-declared-first-pass`.
