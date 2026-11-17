# owner-region-no-edge

ADR 0534: 13 rc.17 owner-warning probes become obligations when no edge proves
the nested function runs under its lexical owner. e6/f26 remain violations, and f18 (`[1].forEach(f)` in a timer) is one since ADR 0561's carrier edge;
e15 is uncertain, e13 silent since ADR 1021 (the literal's only reference is `void o`); e5 and owned twins stay silent. rc.17 types checked. Dormant namespace exports
are pinned by the owner-namespace fixtures.
