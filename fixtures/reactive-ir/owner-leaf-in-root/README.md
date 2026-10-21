# owner-leaf-in-root

**Claim (ADR 0347, amendment of 2026-10-21, leaf regions).** A leaf body inside a `createRoot` is not owned by that root: the leaf's own owner decides what runs there. An `onSettled` under an owner is a leaf owner that forbids children, so an `onCleanup` in it throws `CLEANUP_IN_FORBIDDEN_SCOPE` (SC3001). An `onSettled` inside another leaf body is queued out of band with no owner, so its `onCleanup` raises `NO_OWNER_CLEANUP` (SC4001), and a cleanup it returns raises `SETTLED_CLEANUP_UNOWNED` (SC4001).

Positives (SC4001): `RootNestedSettled` (the inner `onCleanup`), `RootTrackedSettled` (the same inner cleanup under a tracked effect in the root), and `RootSettledCleanup` (the cleanup the queued callback returns).

Controls: `RootOwnedLeaf`, where the owned leaf's `onCleanup` is SC3001 only; `RootOwnCleanup`, where the root's own `onCleanup` is owned and clean; and `RootNestedMemo`, where a `createMemo` in a queued callback needs no owner and is clean.

Checked on the release runtime oracle: `NO_OWNER_CLEANUP` at both nested cleanups, `CLEANUP_IN_FORBIDDEN_SCOPE` at the owned leaf, and no diagnostic at the memo or the root's own cleanup. The settled cleanup raises `SETTLED_CLEANUP_UNOWNED` after its callback returns, so the oracle lists it as unattributed rather than at its line.

Stub declarations are copied from `owner-leaf-nested-cleanup` (published rc.13 declarations).
