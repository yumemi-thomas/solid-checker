# An apply's delivery under an owned flush has that owner

ADR 0639 with ADR 0613, measured in Chrome (`g10`): a `flush()` in a later body drains the apply, its `click()` delivers the JSX handler
under that body's owner, and the handler's write throws. The handler is uncertifiable.

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
