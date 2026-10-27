# program-boundary-workspace

**Claim (ADR 0444).** With no declared boundary, a private package that a sibling of its workspace depends on is open: unseen code in the sibling may call its exports, so `variants/kernel`'s `bump` write stays an `SC2001` obligation. `variants/shell` (private, same files, nobody depends on it) is closed and its write is placed through the handler. `variants/feature` declares the dependency (`workspace:*`) and is no project of its own; the root project has no dependent and is closed.

Stub: rc.17 package manifests at the workspace root.
