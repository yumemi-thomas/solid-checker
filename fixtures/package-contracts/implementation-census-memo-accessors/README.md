# implementation-census-memo-accessors

The generator's half of ADR 0162
(`docs/adr/0162-a-returned-memo-accessor-is-described-by-its-dialect-row.md`):
an export whose every value-carrying completion is, whole, a call spelled
`createMemo` proposes `returns` closed over a described callable that reads a
memo it created (`reads: [owned-memo]`, `returns: [read-value]`), where ADR 0113
would have proposed `plain` for the census to refuse.

| export | proposal | what the census decides |
| --- | --- | --- |
| `createDoubled`, `createLabel` | `read-value` over `owned-memo` | the returned value traces to an audited `createMemo` whole result |
| `createLive` | the same | the computation is the caller's callable; the read may run it, and that run is the registration's, not the read's |
| `createGuarded` | the same | two completions, both a whole `createMemo` call |
| `createShadowed` | the same | **refused by the census**: the spelling only proposes, and the callee is a local function |
| `createEither` | `plain` | a conditional of memos: no literal stated for an arm, so no described shape is proposed |
| `createBound` | `plain` | the memo is bound and written to (`memo.extra = 1`): not a whole call completion |
| `readOnce` | `plain` | the completion is the memo's *value*, not the accessor |

Since ADR 0183 the callback row of each caller's compute handed to
`createMemo` also states the memo's created owner. `createLive`'s row
carries `min: 1`, because its one call covers every completion;
`createGuarded`'s and `createEither`'s two calls are each conditional, so
theirs stay `min: 0`.

This fixture pins generation only. The `solid-js` stub under `node_modules` is
there for dialect selection and is not the audited archive, so no certification
here could use it; the census's own premises are pinned on synthesized
transcripts in `type_facts.rs`'s
`a_memo_is_witnessed_only_for_a_computed_audited_accessor`.
