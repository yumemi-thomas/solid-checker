# Package unknown-callback consumer

This fixture pins the normalized distinction between complete-negative and
unknown callback knowledge.

- `runUnknown(readCount)` must produce one `package-contract-incomplete`
  uncertifiable finding because the reviewed contract marks `callbacks` as
  the callback domain as locally open.
- `noCallback()` uses the same partial summary but passes no potentially
  callable argument, so callback uncertainty is not demanded and stays clean.
- Independently complete-negative read/return/owner/async domains remain
  usable. Unknown callbacks must not turn the entire export into an unknown
  summary.

The declaration is exact for this fixture package; the finding depends on the
runtime contract, not on trusting the declaration as runtime evidence.
