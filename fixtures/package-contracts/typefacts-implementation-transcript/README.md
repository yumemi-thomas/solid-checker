# Authenticated export implementation transcripts

This fixture pins the three execution shapes that policy-2 Type Facts must
distinguish after a demand leaves the declaration-rooted export value:

- `direct` invokes its callback directly inside `try/finally`. The exact call
  remains reachable even though whole-function control-flow completeness is
  deliberately open for `try`.
- `returned` invokes its callback in a named closure which the export returns.
  The return census binds parameter 0 into that executable closure.
- `retained` is the negative control. It contains the same nested invocation,
  but returns a different closure. A captured call that is merely retained in
  the implementation cannot certify callback execution.

The declaration signatures match the runtime API and are accepted by
TypeScript. The checker is proving execution/provenance that those types do not
express; it is not replacing a TypeScript diagnostic.
