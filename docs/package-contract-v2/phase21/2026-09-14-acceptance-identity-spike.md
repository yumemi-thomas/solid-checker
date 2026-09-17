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

## 5. Second pass: what the analyzer can actually re-derive (and the revised cost)

§ 2 said the ingredients for an import-independent identity are "already
signed". True, and not sufficient: the identity also has to be **derivable by
the analyzer**, and that was assumed rather than checked. Checked now, it
changes the cost.

What the analyzer holds for an import at analysis time is
`solid_facts::AttestedImport` — the specifier text, the resolved *declaration*
file (realpath-normalized), its extension, the symlink spelling, and the nearest
manifest's `name`. That is all.

| identity ingredient | analyzer can derive it? |
| --- | --- |
| package name, version | yes, from the nearest installed manifest |
| requested entrypoint | yes, from the specifier |
| package integrity | **npm only today** — `installed_package_integrity` reads `package-lock.json` and npm's hidden lockfile, so a pnpm or bun project has none |
| manifest / declarations file digests | yes, by hashing the resolved files |
| runtime file, export bindings, module closure | **no** — resolution is performed in Node (`artifact-resolution.mjs`); the Rust side only *validates* a supplied `ResolvedImport` |
| `manifestRoot`, `artifactsRoot`, `closureRoot` … | **no** — Merkle families built from the registry archive inside certification |
| **export conditions** | **no** — there are no condition facts at all, in `resolution.rs`, `project.rs` or `diagnostics.rs` |

The last row is the one that decides the shape. Conditions select the artifact:
a contract proven under `import` must not be applied to a consumer that resolved
the same specifier under `require`. So conditions belong in any acceptance
identity — and the analyzer cannot establish its own.

### Revised cost

**B as scoped is not one ADR and a binding.** It is:

1. Type Facts reports the export-condition set it resolved under — a producer
   change and a **handshake protocol bump**.
2. A new import-independent binding in the receipt.
3. `installed_package_integrity` extended past npm, which the lockfile readers
   added by ADR 0108 now make straightforward.
4. Catalog reader and analyzer matching on the new identity.
5. The ADR for the trust posture, plus migration for issued receipts.

### B′, which avoids the protocol bump

Include conditions in the identity, and have the analyzer **match only when the
receipt's condition set is the default single `import`** — refusing to match
anything else rather than guessing. Unmatched imports stay at the acceptance
gate exactly as they are today, so this is a narrowing, not a weakening.

Every certification in the corpus records `exportConditions: ["import"]`, so B′
covers the ordinary case with no producer change, and leaves the protocol bump
for whenever a multi-condition consumer actually needs it.
