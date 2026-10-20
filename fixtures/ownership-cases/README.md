# Ownership cases

The product-owned semantic and TypeScript-ownership gate:

```sh
make ownership-gate
```

Every case materializes `source.prelude + source.text` verbatim. Spans are
relative to `source.text` and use a unique UTF-8 marker (with an explicit
1-based `occurrence` when needed) or a UTF-8 byte `textRange`; the runner adds
the prelude byte length before comparing checker and TypeScript locations.

Expected findings have one of three TypeScript ownership classes:

- `checker-only`: the checker finding must exist and no TypeScript error may
  overlap its span.
- `typescript-owned`: the checker must stay silent and every named TypeScript
  diagnostic must exist at the declared span.
- `distinct-claim`: both diagnostics overlap, and the entry must explain what
  the checker proves that TypeScript does not.

Unlisted findings inside a case's own bytes fail. `count` (positive integer)
covers several findings sharing one exact identity and span. Negative cases
name the rule or family that must remain absent. Cases can also pin per-case
rule options, fix applicability/output, presets, and directly enabled rules.
`migration-ledger.json` reconciles the former upstream cases; the gate requires
zero pending rows.
