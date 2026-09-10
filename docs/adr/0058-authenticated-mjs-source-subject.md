# Authenticated ESM source subjects

The declaration resolver may use the exact `.mjs` source when no matching
declaration branch resolves. This extends the existing JavaScript source
subject lane; it does not synthesize declarations or accept a cross-format
`.d.ts`/`.d.cts` substitute.

Package export selection first runs with the previous declaration rules.
Only a declarations-not-found result permits a second pass with `.mjs`
source enabled. A matching `.d.mts` and a later matching `types` branch keep
their precedence. Null targets, invalid targets, unmatched conditions and
other refusals do not authorize a fallback. The source must exist in the
authenticated archive. CLI selection and native snapshot replay implement
the same ordering and retain the exact selected branch in the resolution trace.

The resulting declaration identity names source bytes and their digest.
Existing export-identity, Type Facts subject, operation, dependency and receipt
verification still apply; selecting a source is not certifying its behavior.
CommonJS and `.mts` fallback behavior is unchanged. No protocol, receipt
format, trust authority, coverage denominator or TypeScript diagnostic changes.

The motivating Testing Library probe previously stopped at
`dom-accessibility-api@0.5.16/dist/index.mjs`. The scoped measurement now
reaches `aria-query@5.3.0`, which has no runtime ESM exports accepted by the
current graph model. Testing Library remains refused; this is a resolved
artifact-selection blocker, not a new certification.
