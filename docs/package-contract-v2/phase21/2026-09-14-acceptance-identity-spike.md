# What an acceptance binds to, and what it would cost to change (2026-09-14)

- **Status:** investigation only. Nothing built, nothing decided.
- **Question:** accepting a certified contract moves no consumer finding,
  because the acceptance index is keyed on `(importer, specifier)` and
  certification binds its acceptance to a synthetic importer it writes itself
  (`2026-09-14-which-closures-change-a-consumer-finding.md` § 7). Two ways out
  were proposed: **(A)** certify for a set of the consumer's importers, or
  **(B)** bind acceptance to the resolved artifact instead of the importing
  file. B was worth a spike because the importer *looked* like a cache key for
  the resolution rather than a property in its own right.
- **Answer:** it is not a cache key. The importer is **signed**. B is a change
  to what the receipt attests, not an analyzer refactor — but it is feasible,
  and every ingredient it needs is already a signed root.

## 1. The importer is inside the signature

Three places, in order:

- `Policy2ReceiptBindings` carries `importer` and `specifier` as fields of the
  signed payload (`contract_certification/policy2_receipt.rs`).
- The catalog reader refuses any entry whose import disagrees with them —
  `bindings.importer != entry.import.importer` is a `ReceiptMismatch`.
- `resolved_import_root`, also signed, is
  `H("solid-checker:policy2-resolved-import:v1" ‖ len ‖ serde_json(ResolvedImport))`,
  and `ResolvedImport` has `importer`, `package_root`, `package_real_root` and
  per-file paths among its fields.

So the resolution identity that the receipt attests is **per-importer and
path-absolute** by construction. An analyzer cannot match a consumer's import
against it, however it is indexed.

`Policy2Portable` does not help, and it is the obvious false lead: it selects
the *issuer* trust chain (Ed25519 against a trust store, rather than a
persistent-local scope seed), not what the receipt binds to. Portable and
persistent-local receipts carry the same importer binding.

## 2. Why B is nevertheless feasible

The receipt already signs a set of roots that are **content** Merkle roots and
mention no importer and no absolute path:

`package_root`, `manifest_root`, `artifacts_root`, `declarations_root`,
`exports_root`, `closure_root`, `transform_root`, `semantic_digest`.

An import-independent acceptance identity is a selection from those, plus the
package name, version, integrity, requested entrypoint and export conditions.
Nothing new has to be computed at certification time; what is missing is a
binding that *commits* to that selection separately from `resolved_import_root`.

## 3. The soundness argument B would have to make

The importer binding defends against one thing: the same specifier resolving to
a different artifact from a different file, which node resolution genuinely
allows (nested `node_modules`, conditions, self-reference).

Matching on content identity addresses that directly rather than by proxy. If
the analyzer resolves the consumer's import with its own resolver and the
resulting artifact identity equals the attested one, the import demonstrably
reached the same bytes — which is the property the importer was standing in for,
established rather than assumed.

The weakening to argue about is not that. It is **what the analyzer's own
resolution is worth**: today the receipt carries a resolution performed by the
certifier, and B would have the analyzer perform one and compare. That is a
different trust posture, and it is the thing an ADR has to justify, not the
indexing change.

Two smaller matters, both real:

- **Absolute paths.** `package_root` inside `ResolvedImport` differs between a
  certification tree and a consumer tree whenever they are not the same tree.
  `rebase_catalog_import` handles catalog-relative paths, not this. Any
  import-independent identity has to exclude them, which is a reason to commit
  to a *new* root rather than reuse `resolved_import_root`.
- **Conditions.** Export conditions select the artifact, so they belong in the
  identity; two consumers importing under different conditions must not share
  an acceptance.

## 4. Cost, and the alternative

**B:** one ADR, a new signed binding, an analyzer-side derivation, and a
migration for existing receipts. Bounded, but it touches the trust boundary.

**A** (certify for the consumer's importers) changes no trust model and could
be built now — `certify-contract.mjs` would take the consumer project and
enumerate its importing files. The ergonomics are the objection, and they are
not small: `kobalte/packages/core` alone needs 251 entries, they go stale on
every new import, and the catalog stops being reviewable.

Neither is chosen here. What this spike settles is that B cannot be done as a
quiet indexing change, and that its real argument is about the analyzer's
resolver rather than about cache keys.
