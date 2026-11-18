# A `remove` of a created anchor is a hazard

ADR 0642: nothing proves an element has no focus to lose (`autofocus`, a selection or the tab key can give it one no use names), so
ADR 0640's no-listener exclusion is for `click` alone, and the `remove` still withholds listener and flush proofs. ADR 1099: the
anchor's path (anchor, `body`, `html`) holds no JSX element, so neither call reaches a JSX handler; the handlers' owner and read
proofs no longer wait on them (the counter handler's SC2001 and SC1001 rows are gone). `rc21-created-anchor` is the same program
without the `remove`.
