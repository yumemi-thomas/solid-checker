# caught-handed-read

Claim (ADR 0838, R2h): a `try` around a pending read swallows its not-ready throw wherever the carrier that runs the function stands, so SC5003 is not proved there (Chrome rc.21 reports CAUGHT_REACTIVE_READ only); the read is uncertifiable, blocker `async-read-caught`. Positives (uncertifiable, never a violation): `CaughtHandedRead` (`forEach`), `CaughtCalledRead` (`.call`). Negative twin: `HandedRead`, the same carrier with no `try`, keeps its SC5003 violation. Stubs copied from `async-boundary-call-attributed` (rc.21 `createMemo`, `render`).
