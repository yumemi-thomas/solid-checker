# Package unknown-callback consumer

This fixture pins the schema-v1 distinction between proven-none and unknown
callback claims.

- `runUnknown(readCount)` must produce one `package-contract-incomplete`
  uncertifiable finding because the reviewed contract marks `callbacks` as
  `{ "status": "unknown" }`.
- `noCallback()` uses the same partial summary but passes no potentially
  callable argument, so callback uncertainty is not demanded and stays clean.
- The contract's omitted read/return/owner/async fields remain reviewed
  negative claims. Unknown callbacks must not turn the entire export into an
  unknown summary.

The declaration is exact for this fixture package; the finding depends on the
runtime contract, not on trusting the declaration as runtime evidence.
