# owner-leaf-in-root

**Claim (ADR 0347, leaf regions).** A leaf body inside a `createRoot` is not owned by that root; its own owner decides. An `onSettled` under an owner forbids children, so an `onCleanup` in it throws `CLEANUP_IN_FORBIDDEN_SCOPE` (SC3001). An `onSettled` inside another leaf body is queued with no owner: its `onCleanup` raises `NO_OWNER_CLEANUP`, and a cleanup it returns raises `SETTLED_CLEANUP_UNOWNED` (SC4001).

Positives (SC4001): `RootNestedSettled`, `RootTrackedSettled`, `RootSettledCleanup`.

Controls: `RootOwnedLeaf` (SC3001 only), `RootOwnCleanup` and `RootNestedMemo` (clean).

Checked on the release runtime oracle: the codes above fire at the sites named, and nothing fires at the memo or the root's own cleanup.

Stubs: copied from `owner-leaf-nested-cleanup`.
