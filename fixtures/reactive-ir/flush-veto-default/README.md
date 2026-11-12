# flush-veto-default

**Claim (ADR 0613).** A `flush()` in a parameter default is in no function's body and is not module evaluation: it runs at its function's call, under that caller's owner. The census gives it no owner-free class, so it vetoes, and the apply's write (which rc.17 rejects there) is uncertifiable, not certified.

ADR 0961 (PD): the flush in the default (line 5) is itself an SC2008 uncertifiable row (`parameter-initializer-unplaced`): a default runs when a call omits the argument, `drain()` here is called in a component body, and no net reads which calls omit it, so it may run on an effect's apply.

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
